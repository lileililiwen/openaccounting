//! Foreign-exchange rate store and derivation (`multi-currency-fx`).
//!
//! Rates live in `fx_rates` as `(base, quote, rate, rate_date)` where
//! `rate` is the amount of `quote` per ONE unit of `base`. Lookups take
//! the most recent rate on or before the transaction date; a direct pair
//! is preferred, otherwise the inverse pair is used (computed, never
//! stored). Missing pairs within the 90-day lookback window are a hard
//! error naming the pair and date.
//!
//! Base amounts are derived at save time and frozen on the posting —
//! later rate edits never restate history (matches `append-only-mode`).

use chrono::{Datelike, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

/// How far back a rate may sit from the transaction date.
pub const LOOKBACK_DAYS: i64 = 90;

/// The currency exponent used for base-amount rounding. All seeded
/// ledgers use 2-decimal currencies; DECIMAL(20,4) columns keep more
/// precision internally but derived amounts round half-up to cents.
pub const BASE_EXPONENT: u32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum FxError {
    #[error("No {base} → {quote} exchange rate on or before {date} within {LOOKBACK_DAYS} days")]
    MissingRate {
        base: String,
        quote: String,
        date: NaiveDate,
    },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Look up the applicable rate for `(base → quote)` on `date`.
///
/// Returns the rate as quote-per-base. Tries the direct pair first,
/// then the inverse (computed, never stored). Rates are committed data,
/// so reading from the pool outside the write transaction is safe.
pub async fn lookup(
    pool: &PgPool,
    base: &str,
    quote: &str,
    date: NaiveDate,
) -> Result<Decimal, FxError> {
    if base == quote {
        return Ok(Decimal::ONE);
    }

    let rate: Option<(Decimal,)> = sqlx::query_as(
        "SELECT rate FROM fx_rates
         WHERE ((base_currency = $1 AND quote_currency = $2)
                OR (base_currency = $2 AND quote_currency = $1))
               AND rate_date <= $3
         ORDER BY rate_date DESC
         LIMIT 1",
    )
    .bind(base)
    .bind(quote)
    .bind(date)
    .fetch_optional(pool)
    .await?;
    let Some((stored,)) = rate else {
        return Err(FxError::MissingRate {
            base: base.to_string(),
            quote: quote.to_string(),
            date,
        });
    };

    if stored == Decimal::ZERO {
        return Err(FxError::MissingRate {
            base: base.to_string(),
            quote: quote.to_string(),
            date,
        });
    }

    let (row_base,): (String,) = sqlx::query_as(
        "SELECT base_currency FROM fx_rates
         WHERE ((base_currency = $1 AND quote_currency = $2)
                OR (base_currency = $2 AND quote_currency = $1))
               AND rate_date <= $3
         ORDER BY rate_date DESC
         LIMIT 1",
    )
    .bind(base)
    .bind(quote)
    .bind(date)
    .fetch_one(pool)
    .await?;

    if row_base == base {
        Ok(stored)
    } else {
        Ok(Decimal::ONE / stored)
    }
}

/// Derive the base-currency amount from a foreign amount at `rate`,
/// rounded half-up (away from zero) to the base currency exponent.
/// The sign of the foreign amount is preserved.
pub fn derive_base_amount(foreign_amount: Decimal, rate: Decimal) -> Decimal {
    let raw = foreign_amount * rate;
    raw.round_dp_with_strategy(BASE_EXPONENT, RoundingStrategy::MidpointAwayFromZero)
}

/// Parse one ECB historical CSV (`eurofxref-hist` layout) into
/// `(quote_currency, rate, date)` triples with EUR as the base.
///
/// Header: `Date,USD,JPY,...`; rows are dated descending. Plain
/// unquoted CSV with dot decimals (the published format). Empty cells
/// (holidays per currency) and malformed rows are skipped silently so
/// one bad bank export never poisons the store. Parsing is pure;
/// upserting is the caller's job, which keeps the worker idempotent
/// via `ON CONFLICT DO NOTHING`.
pub fn parse_ecb_csv(csv: &str) -> Vec<(String, Decimal, NaiveDate)> {
    let mut out = Vec::new();
    let mut lines = csv.lines();
    let Some(header) = lines.next() else {
        return out;
    };
    let currencies: Vec<&str> = header
        .split(',')
        .skip(1)
        .map(str::trim)
        .filter(|c| c.len() == 3 && c.chars().all(|c| c.is_ascii_uppercase()))
        .collect();

    for line in lines {
        let mut cols = line.split(',');
        let Some(date_raw) = cols.next() else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(date_raw.trim(), "%Y-%m-%d") else {
            continue;
        };
        for (cur, cell) in currencies.iter().zip(cols.by_ref()) {
            let cell = cell.trim();
            if cell.is_empty() {
                continue;
            }
            if let Ok(rate) = cell.parse::<Decimal>() {
                if rate > Decimal::ZERO {
                    out.push(((*cur).to_string(), rate, date));
                }
            }
        }
    }
    out
}

/// Upsert parsed ECB rates. Idempotent: rows already present (manual or
/// feed) are left untouched — manual entry always wins because only the
/// feed uses `ON CONFLICT DO NOTHING`.
pub async fn upsert_ecb_rates(
    pool: &PgPool,
    rates: &[(String, Decimal, NaiveDate)],
) -> Result<u64, sqlx::Error> {
    let mut inserted: u64 = 0;
    let mut tx = pool.begin().await?;
    for (quote, rate, date) in rates {
        let res = sqlx::query(
            "INSERT INTO fx_rates (base_currency, quote_currency, rate, rate_date, source)
             VALUES ('EUR', $1, $2, $3, 'ecb')
             ON CONFLICT (base_currency, quote_currency, rate_date) DO NOTHING",
        )
        .bind(quote)
        .bind(rate)
        .bind(date)
        .execute(&mut *tx)
        .await?;
        inserted += res.rows_affected();
    }
    tx.commit().await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn derive_rounds_half_up_to_cents() {
        // 1000 EUR × 0.91 → exactly 910.00
        assert_eq!(
            derive_base_amount(Decimal::new(1000, 0), Decimal::new(91, 2)),
            Decimal::new(91000, 2)
        );
        // Half case rounds away from zero: 0.915 × 1000 = 915.00
        assert_eq!(
            derive_base_amount(Decimal::new(1000, 0), Decimal::new(915, 3)),
            Decimal::new(91500, 2)
        );
        // Third decimal forces a real rounding decision: 333.333 → 333.33
        assert_eq!(
            derive_base_amount(Decimal::new(1000, 0), Decimal::new(333333, 6)),
            Decimal::new(33333, 2)
        );
        // Sign preserved.
        assert_eq!(
            derive_base_amount(Decimal::new(-1000, 0), Decimal::new(91, 2)),
            Decimal::new(-91000, 2)
        );
    }

    #[test]
    fn ecb_csv_parses_fixture_rows() {
        let csv = "Date,USD,JPY,BGN\n2026-08-21,1.0891,168.42,1.9558\n2026-08-20,1.0877,,1.9558\n";
        let rows = parse_ecb_csv(csv);
        assert_eq!(rows.len(), 5, "BGN repeats + one empty JPY cell skipped");
        let usd = rows.iter().find(|(c, _, _)| c == "USD").unwrap();
        assert_eq!(usd.0, "USD");
        assert_eq!(usd.1, Decimal::new(10891, 4));
        assert_eq!(usd.2.year(), 2026);
        assert_eq!(usd.2.month(), 8);
        // The empty JPY cell on 08-20 must not produce a row.
        assert!(
            rows.iter()
                .filter(|(c, _, d)| c == "JPY" && d.day() == 20)
                .count()
                == 0
        );
    }

    #[test]
    fn ecb_csv_skips_garbage_rows() {
        let csv = "Date,USD\nnot-a-date,1.0\n2026-08-21,x\n2026-08-22,1.0912\n";
        let rows = parse_ecb_csv(csv);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, Decimal::new(10912, 4));
    }
}

/// Result of running a month-end revaluation for one account.
#[derive(Debug, Clone)]
pub struct RevaluationPosted {
    pub account_id: Uuid,
    pub currency: String,
    /// Signed gain (>0) or loss (<0) posted to the P&L, in base currency.
    pub fx_gain_loss: Decimal,
    pub transaction_id: Uuid,
}

/// Run month-end revaluation for every foreign-currency monetary
/// account of `ledger_id` that has no revaluation row yet for
/// `period_month` (`multi-currency-fx`).
///
/// For each account: current value = foreign balance × rate at `date`;
/// book value = Σ signed base amounts of the foreign postings. The
/// difference posts as a balanced transaction against an auto-created
/// "FX Unrealized Gain/Loss" income account (gains) / "FX Unrealized
/// Loss" expense account (losses). Accounts with a zero difference are
/// skipped entirely (no row, no posting).
///
/// Returns the posted revaluations. When every candidate account was
/// already revalued for the month, returns `Err(AlreadyRevalued)`.
#[derive(Debug, thiserror::Error)]
pub enum RevaluationError {
    #[error("FX revaluation for {month} has already been run for every eligible account")]
    AlreadyRevalued { month: String },
    #[error("{0}")]
    Fx(#[from] FxError),
    #[error(transparent)]
    Posting(#[from] crate::domain::posting_service::PostingServiceError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub async fn run_revaluation(
    pool: &PgPool,
    ledger_id: Uuid,
    date: NaiveDate,
    actor: Uuid,
) -> Result<Vec<RevaluationPosted>, RevaluationError> {
    let period_month = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    let month_label = period_month.format("%Y-%m").to_string();

    // Foreign-currency monetary accounts with non-zero foreign balances.
    let candidates: Vec<(Uuid, String, Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT a.id,
                  a.currency,
                  COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.foreign_amount
                                    ELSE -p.foreign_amount END), 0) AS foreign_balance,
                  COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount
                                    ELSE -p.amount END), 0) AS book_value
           FROM accounts a
           JOIN postings p ON p.account_id = a.id AND p.foreign_amount IS NOT NULL
           WHERE a.ledger_id = $1
                 AND a.type IN ('ASSET', 'LIABILITY')
                 AND a.currency <> (SELECT base_currency FROM ledgers WHERE id = $1)
                 AND a.is_archived = FALSE
           GROUP BY a.id, a.currency
           HAVING COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.foreign_amount
                                    ELSE -p.foreign_amount END), 0) <> 0"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;

    // Drop accounts already revalued for this month.
    let mut pending = Vec::new();
    for row in candidates {
        let done: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM fx_revaluations
             WHERE ledger_id = $1 AND account_id = $2 AND period_month = $3",
        )
        .bind(ledger_id)
        .bind(row.0)
        .bind(period_month)
        .fetch_optional(pool)
        .await?;
        if done.is_none() {
            pending.push(row);
        }
    }
    if pending.is_empty() {
        return Err(RevaluationError::AlreadyRevalued { month: month_label });
    }

    let mut posted = Vec::new();
    for (account_id, currency, foreign_balance, book_value) in pending {
        let rate = lookup(
            pool,
            &currency,
            &base_currency_of(pool, ledger_id).await?,
            date,
        )
        .await?;
        let new_value = derive_base_amount(foreign_balance, rate);
        let diff = new_value - book_value;
        if diff == Decimal::ZERO {
            continue;
        }

        let (gain_account, loss_account) = ensure_fx_accounts(pool, ledger_id, actor).await?;
        let (debit_account, credit_account) = if diff > Decimal::ZERO {
            (account_id, gain_account)
        } else {
            (loss_account, account_id)
        };
        let amount = diff.abs();

        let created = crate::domain::posting_service::PostingService::create(
            pool,
            crate::domain::posting_service::NewTransaction {
                ledger_id,
                txn_date: date,
                description: format!("FX revaluation {month_label} ({currency})"),
                payee: None,
                reference: None,
                kind: None,
                created_by: actor,
                reverses_id: None,
                number: None,
                tax_links: vec![],
                lines: vec![
                    crate::domain::TxnLineInput {
                        account_id: debit_account,
                        signed_amount: amount,
                        memo: Some(format!("FX revaluation {currency}")),
                        tax_rate_id: None,
                        foreign: None,
                    },
                    crate::domain::TxnLineInput {
                        account_id: credit_account,
                        signed_amount: -amount,
                        memo: Some(format!("FX revaluation {currency}")),
                        tax_rate_id: None,
                        foreign: None,
                    },
                ],
            },
        )
        .await?;

        sqlx::query(
            "INSERT INTO fx_revaluations
                (ledger_id, account_id, period_month, transaction_id, fx_gain_loss, posted_by)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(ledger_id)
        .bind(account_id)
        .bind(period_month)
        .bind(created.id)
        .bind(diff)
        .bind(actor)
        .execute(pool)
        .await?;

        posted.push(RevaluationPosted {
            account_id,
            currency,
            fx_gain_loss: diff,
            transaction_id: created.id,
        });
    }

    Ok(posted)
}

async fn base_currency_of(pool: &PgPool, ledger_id: Uuid) -> Result<String, sqlx::Error> {
    let (c,): (String,) = sqlx::query_as("SELECT base_currency FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(pool)
        .await?;
    Ok(c)
}

/// Find or create the auto-generated FX P&L pair for a ledger.
/// Returns `(gain_income_account_id, loss_expense_account_id)`.
async fn ensure_fx_accounts(
    pool: &PgPool,
    ledger_id: Uuid,
    actor: Uuid,
) -> Result<(Uuid, Uuid), sqlx::Error> {
    async fn ensure_one(
        pool: &PgPool,
        ledger_id: Uuid,
        name: &str,
        acc_type: &str,
        subtype: &str,
    ) -> Result<Uuid, sqlx::Error> {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
                .bind(ledger_id)
                .bind(name)
                .fetch_optional(pool)
                .await?;
        if let Some((id,)) = existing {
            return Ok(id);
        }
        let code: Option<String> = {
            let (max_code,): (Option<i64>,) = sqlx::query_as(
                "SELECT MAX(NULLIF(code, '')::BIGINT) FROM accounts
                 WHERE ledger_id = $1 AND code IS NOT NULL AND code ~ '^[0-9]+$'",
            )
            .bind(ledger_id)
            .fetch_one(pool)
            .await?;
            max_code.map(|m| format!("{}", m + 1))
        };
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO accounts (ledger_id, name, type, subtype, currency, code)
             VALUES ($1, $2, $3, $4, (SELECT base_currency FROM ledgers WHERE id = $1), $5)
             RETURNING id",
        )
        .bind(ledger_id)
        .bind(name)
        .bind(acc_type)
        .bind(subtype)
        .bind(&code)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    let gain = ensure_one(
        pool,
        ledger_id,
        "FX Unrealized Gain/Loss",
        "INCOME",
        "OPERATING_INCOME",
    )
    .await?;
    let loss = ensure_one(
        pool,
        ledger_id,
        "FX Unrealized Loss",
        "EXPENSE",
        "OPERATING_EXPENSE",
    )
    .await?;
    Ok((gain, loss))
}
