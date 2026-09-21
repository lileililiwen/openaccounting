//! Hard-close watermark, maker-checker, and gapless invoice sequencing
//! (`pro-close-controls`).
//!
//! Pure helpers plus small SQL helpers. All write paths consult
//! [`closed_through_for`] before posting; the watermark is a single
//! date per ledger (`closed_periods.closed_through`, with legacy
//! `period_year` rows honored as Dec-31 of that year).

use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Minimum reopen/override/void reason length (spec: min 10 chars).
pub const MIN_REASON_LEN: usize = 10;

/// Validate a reopen/override/void reason.
pub fn validate_reason(reason: &str) -> Result<String, String> {
    let trimmed = reason.trim().to_string();
    if trimmed.chars().count() < MIN_REASON_LEN {
        return Err(format!(
            "Reason must be at least {MIN_REASON_LEN} characters."
        ));
    }
    Ok(trimmed)
}

/// Return the active close watermark for a ledger, if any.
///
/// Prefers `MAX(closed_through)`; falls back to legacy `period_year`
/// rows (interpreted as Dec-31 of that year). Returns `None` when the
/// ledger has never been closed.
pub async fn closed_through_for(
    pool: &PgPool,
    ledger_id: Uuid,
) -> Result<Option<NaiveDate>, sqlx::Error> {
    let row: Option<(Option<NaiveDate>, Option<i32>)> = sqlx::query_as(
        r#"SELECT MAX(closed_through),
                  MAX(period_year)
           FROM closed_periods WHERE ledger_id = $1"#,
    )
    .bind(ledger_id)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        None => None,
        Some((through, year)) => {
            if let Some(d) = through {
                Some(d)
            } else if let Some(y) = year {
                NaiveDate::from_ymd_opt(y, 12, 31)
            } else {
                None
            }
        }
    })
}

/// Same as [`closed_through_for`] but inside an open transaction.
pub async fn closed_through_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    ledger_id: Uuid,
) -> Result<Option<NaiveDate>, sqlx::Error> {
    let row: Option<(Option<NaiveDate>, Option<i32>)> = sqlx::query_as(
        r#"SELECT MAX(closed_through),
                  MAX(period_year)
           FROM closed_periods WHERE ledger_id = $1"#,
    )
    .bind(ledger_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(match row {
        None => None,
        Some((through, year)) => {
            if let Some(d) = through {
                Some(d)
            } else if let Some(y) = year {
                NaiveDate::from_ymd_opt(y, 12, 31)
            } else {
                None
            }
        }
    })
}

/// True when `date` falls inside the closed range (`date <= watermark`).
pub fn is_closed(watermark: Option<NaiveDate>, date: NaiveDate) -> bool {
    match watermark {
        Some(w) => date <= w,
        None => false,
    }
}

/// Fetch the ledger's maker-checker threshold. `None` (NULL/negative)
/// disables maker-checker.
pub async fn approval_threshold(
    pool: &PgPool,
    ledger_id: Uuid,
) -> Result<Option<rust_decimal::Decimal>, sqlx::Error> {
    let v: Option<rust_decimal::Decimal> =
        sqlx::query_scalar("SELECT approval_threshold FROM ledgers WHERE id = $1")
            .bind(ledger_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    Ok(v.filter(|t| *t >= rust_decimal::Decimal::ZERO))
}

/// Allocate the next gapless invoice number for `(ledger, year)`.
///
/// Format is `{year}-{NNNN}` with `NNNN` zero-padded to 4 digits
/// (e.g. `2026-0042`). Holds a row lock on `invoice_sequences` so
/// concurrent allocations serialize; seeds from existing
/// `invoices.invoice_number` maxima on first use.
pub async fn next_invoice_number(
    tx: &mut Transaction<'_, Postgres>,
    ledger_id: Uuid,
    year: i32,
) -> Result<String, sqlx::Error> {
    let existing: Option<(i32,)> = sqlx::query_as(
        "SELECT last_no FROM invoice_sequences
         WHERE ledger_id = $1 AND year = $2 FOR UPDATE",
    )
    .bind(ledger_id)
    .bind(year)
    .fetch_optional(&mut **tx)
    .await?;
    let mut last = match existing {
        Some((n,)) => n,
        None => {
            // Seed from the largest existing suffix so the backfill
            // never reuses a number.
            let max_suffix: Option<i32> = sqlx::query_scalar(
                r#"SELECT MAX(substring(invoice_number from '-(\d+)$')::INT)
                   FROM invoices
                   WHERE ledger_id = $1
                     AND invoice_number LIKE $2"#,
            )
            .bind(ledger_id)
            .bind(format!("{year}-%"))
            .fetch_one(&mut **tx)
            .await?;
            let seed = max_suffix.unwrap_or(0);
            sqlx::query(
                "INSERT INTO invoice_sequences (ledger_id, year, last_no)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (ledger_id, year) DO NOTHING",
            )
            .bind(ledger_id)
            .bind(year)
            .bind(seed)
            .execute(&mut **tx)
            .await?;
            // Re-read under lock (another writer may have won).
            let n: Option<(i32,)> = sqlx::query_as(
                "SELECT last_no FROM invoice_sequences
                 WHERE ledger_id = $1 AND year = $2 FOR UPDATE",
            )
            .bind(ledger_id)
            .bind(year)
            .fetch_optional(&mut **tx)
            .await?;
            n.map(|r| r.0).unwrap_or(seed)
        }
    };
    last += 1;
    sqlx::query(
        "UPDATE invoice_sequences SET last_no = $3
         WHERE ledger_id = $1 AND year = $2",
    )
    .bind(ledger_id)
    .bind(year)
    .bind(last)
    .execute(&mut **tx)
    .await?;
    Ok(format!("{year}-{last:04}"))
}

/// Gap report: `(allocated_numbers, void_numbers, missing_numbers)`.
///
/// `missing` = sequence integers in `[1, max]` never assigned to any
/// invoice; `void` = assigned numbers whose invoice row is voided
/// (retained, not missing).
pub async fn invoice_gap_report(
    pool: &PgPool,
    ledger_id: Uuid,
    year: i32,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT invoice_number, status FROM invoices
           WHERE ledger_id = $1 AND invoice_number LIKE $2"#,
    )
    .bind(ledger_id)
    .bind(format!("{year}-%"))
    .fetch_all(pool)
    .await?;
    let mut used: std::collections::BTreeMap<i32, String> = Default::default();
    let mut voids: Vec<String> = Vec::new();
    for (num, status) in &rows {
        if let Some(suffix) = num.strip_prefix(&format!("{year}-")) {
            if let Ok(n) = suffix.parse::<i32>() {
                used.insert(n, num.clone());
                if status == "void" {
                    voids.push(num.clone());
                }
            }
        }
    }
    let max = used.keys().next_back().copied().unwrap_or(0);
    let mut missing = Vec::new();
    for n in 1..=max {
        if !used.contains_key(&n) {
            missing.push(format!("{year}-{n:04}"));
        }
    }
    let allocated: Vec<String> = used.into_values().collect();
    Ok((allocated, voids, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_watermark_rejects_closed_dates_and_accepts_later() {
        let watermark = NaiveDate::from_ymd_opt(2026, 3, 31);
        assert!(is_closed(
            watermark,
            NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
        ));
        assert!(is_closed(
            watermark,
            NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
        ));
        assert!(!is_closed(
            watermark,
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()
        ));
        assert!(!is_closed(
            None,
            NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
        ));
    }

    #[test]
    fn reason_validation_requires_ten_chars() {
        assert!(validate_reason("").is_err());
        assert!(validate_reason("short").is_err());
        assert!(validate_reason("  duplicate issue, re-bill  ").is_ok());
    }
}
