//! Plain-text accounting (PTA) import orchestration
//! (`d3-plaintext-export`).
//!
//! Both importers (Beancount and hledger-style CSV) parse into the
//! same [`PtaTxn`] shape, then this module resolves accounts, checks
//! balance, de-duplicates against existing rows, and inserts.

use std::collections::HashSet;

use chrono::Datelike;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::Direction;

/// One posting leg of a parsed plain-text transaction. The amount
/// is stored positive; the sign that carried it in the source file
/// selects the direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtaPosting {
    pub account_name: String,
    pub amount: Decimal,
    pub direction: Direction,
}

/// A parsed plain-text transaction, independent of source format.
#[derive(Debug, Clone)]
pub struct PtaTxn {
    pub date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub postings: Vec<PtaPosting>,
}

impl PtaTxn {
    /// The `(date, description, payee, total)` fingerprint used to
    /// detect already-imported transactions. `total` is the sum of
    /// the absolute posting amounts, which is stable across both
    /// source formats and matches the DB-side `SUM(p.amount)`.
    pub fn fingerprint(&self) -> (NaiveDate, String, String, Decimal) {
        let total: Decimal = self.postings.iter().map(|p| p.amount).sum();
        (
            self.date,
            self.description.clone(),
            self.payee.clone().unwrap_or_default(),
            total,
        )
    }

    /// One-line human description for the dry-run / import report.
    fn describe(&self) -> String {
        format!(
            "{} {} {} ({} postings)",
            self.date,
            self.payee.as_deref().unwrap_or(""),
            self.description,
            self.postings.len()
        )
    }
}

/// Supported plain-text formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtaFormat {
    Beancount,
    HledgerCsv,
}

impl PtaFormat {
    /// Parse the `--format` CLI value.
    pub fn from_arg(s: &str) -> anyhow::Result<Self> {
        match s {
            "beancount" => Ok(Self::Beancount),
            "hledger-csv" | "hledger" => Ok(Self::HledgerCsv),
            other => {
                anyhow::bail!("unknown format '{other}' (expected 'beancount' or 'hledger-csv')")
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Beancount => "beancount",
            Self::HledgerCsv => "hledger-csv",
        }
    }
}

/// Result of an import run. `planned` carries one line per parsed
/// transaction so `--dry-run` can print the diff.
#[derive(Debug, Default)]
pub struct ImportReport {
    pub inserted: usize,
    pub skipped: usize,
    pub dry_run: bool,
    pub planned: Vec<String>,
    pub errors: Vec<String>,
}

/// Parse `input` in `format`, then insert the transactions into
/// `ledger_id`. `actor_id` is recorded as the author on inserted
/// transactions and audit entries.
///
/// When `dry_run` is set nothing is written; the report's `planned`
/// lines describe exactly what would happen.
pub async fn import(
    pool: &PgPool,
    ledger_id: Uuid,
    actor_id: Uuid,
    format: PtaFormat,
    input: &str,
    dry_run: bool,
) -> anyhow::Result<ImportReport> {
    let txns = match format {
        PtaFormat::Beancount => super::beancount::parse(input)?,
        PtaFormat::HledgerCsv => super::hledger::parse(input)?,
    };

    let ledger =
        sqlx::query_as::<_, (String,)>(r#"SELECT base_currency FROM ledgers WHERE id = $1"#)
            .bind(ledger_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ledger {ledger_id} not found"))?;
    let base_currency = ledger.0;

    let accounts: Vec<(Uuid, String)> =
        sqlx::query_as(r#"SELECT id, name FROM accounts WHERE ledger_id = $1"#)
            .bind(ledger_id)
            .fetch_all(pool)
            .await?;

    let existing = existing_fingerprints(pool, ledger_id).await?;

    let mut report = ImportReport {
        dry_run,
        ..Default::default()
    };

    for (i, txn) in txns.iter().enumerate() {
        let mut resolved: Vec<(Uuid, &PtaPosting)> = Vec::with_capacity(txn.postings.len());
        let mut unresolved: Vec<String> = Vec::new();
        for p in &txn.postings {
            match resolve_account(&accounts, &p.account_name) {
                Some(id) => resolved.push((id, p)),
                None => unresolved.push(p.account_name.clone()),
            }
        }
        if !unresolved.is_empty() {
            report.errors.push(format!(
                "entry {}: unknown account(s): {}",
                i + 1,
                unresolved.join(", ")
            ));
            continue;
        }

        let signed_sum: Decimal = txn
            .postings
            .iter()
            .map(|p| match p.direction {
                Direction::Debit => p.amount,
                Direction::Credit => -p.amount,
            })
            .sum();
        if signed_sum != Decimal::ZERO {
            report.errors.push(format!(
                "entry {}: postings do not balance (net {})",
                i + 1,
                signed_sum
            ));
            continue;
        }

        if existing.contains(&txn.fingerprint()) {
            report.skipped += 1;
            report
                .planned
                .push(format!("skip (duplicate): {}", txn.describe()));
            continue;
        }

        if dry_run {
            report.planned.push(format!("insert: {}", txn.describe()));
            continue;
        }

        match insert_txn(pool, ledger_id, actor_id, txn, &resolved, &base_currency).await {
            Ok(()) => {
                report.inserted += 1;
                report.planned.push(format!("inserted: {}", txn.describe()));
            }
            Err(e) => report.errors.push(format!("entry {}: {e}", i + 1)),
        }
    }

    Ok(report)
}

/// Resolve a source account name to an account id in this ledger.
///
/// Candidates tried in order:
/// 1. the exact name,
/// 2. the tail of a Beancount-style type root (`Assets:Cash` → `Cash`),
/// 3. the reverse of the exporter's mangling (`Cash_on_Hand` →
///    `Cash on Hand`).
///
/// Each candidate is also tried re-mangled, in case a stored name
/// genuinely contains the replaced characters.
fn resolve_account(accounts: &[(Uuid, String)], name: &str) -> Option<Uuid> {
    let mut candidates = vec![name.to_string()];
    if let Some((_, tail)) = name.rsplit_once(':') {
        candidates.push(tail.to_string());
    }
    candidates.push(crate::export::beancount::unmangle_account_name(name));

    for c in &candidates {
        if let Some((id, _)) = accounts.iter().find(|(_, n)| n == c) {
            return Some(*id);
        }
        let mangled: String = c
            .chars()
            .map(|ch| match ch {
                ' ' | '\t' | '-' | ':' => '_',
                other => other,
            })
            .collect();
        if let Some((id, _)) = accounts.iter().find(|(_, n)| n == &mangled) {
            return Some(*id);
        }
    }
    None
}

/// Load the fingerprint set of every existing transaction in the
/// ledger, mirroring [`PtaTxn::fingerprint`].
async fn existing_fingerprints(
    pool: &PgPool,
    ledger_id: Uuid,
) -> Result<HashSet<(NaiveDate, String, String, Decimal)>, sqlx::Error> {
    let rows: Vec<(NaiveDate, String, Option<String>, Decimal)> = sqlx::query_as(
        r#"SELECT t.txn_date, t.description, t.payee, COALESCE(SUM(p.amount), 0)
           FROM transactions t JOIN postings p ON p.transaction_id = t.id
           WHERE t.ledger_id = $1
           GROUP BY t.id, t.txn_date, t.description, t.payee"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(d, s, payee, total)| (d, s, payee.unwrap_or_default(), total))
        .collect())
}

/// Insert one balanced transaction and its postings, honouring the
/// closed-period rule the web UI enforces.
async fn insert_txn(
    pool: &PgPool,
    ledger_id: Uuid,
    actor_id: Uuid,
    txn: &PtaTxn,
    resolved: &[(Uuid, &PtaPosting)],
    base_currency: &str,
) -> anyhow::Result<()> {
    let is_closed: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM closed_periods
             WHERE ledger_id = $1 AND period_year = $2)"#,
    )
    .bind(ledger_id)
    .bind(txn.date.year())
    .fetch_one(pool)
    .await?;
    if is_closed {
        anyhow::bail!("period {} is closed", txn.date.year());
    }

    let mut db_tx = pool.begin().await?;
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, currency, kind, created_by)
           VALUES ($1, $2, $3, $4, NULL, $5, 'standard', $6)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(txn.date)
    .bind(&txn.description)
    .bind(txn.payee.as_deref().filter(|s| !s.is_empty()))
    .bind(base_currency)
    .bind(actor_id)
    .fetch_one(&mut *db_tx)
    .await?;

    for (account_id, p) in resolved {
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
               VALUES ($1, $2, $3, $4, NULL)"#,
        )
        .bind(txn_id)
        .bind(account_id)
        .bind(p.amount)
        .bind(p.direction.as_str())
        .execute(&mut *db_tx)
        .await?;
    }
    db_tx.commit().await?;

    let _ = crate::audit::log(
        pool,
        Some(ledger_id),
        actor_id,
        "import",
        "transaction",
        Some(txn_id),
        None,
        Some(serde_json::json!({
            "source": "pta",
            "description": txn.description,
            "date": txn.date,
        })),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_sums_absolute_amounts() {
        let txn = PtaTxn {
            date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            description: "Coffee".into(),
            payee: Some("Mogador".into()),
            postings: vec![
                PtaPosting {
                    account_name: "Cash".into(),
                    amount: Decimal::new(4250, 2),
                    direction: Direction::Debit,
                },
                PtaPosting {
                    account_name: "Expenses".into(),
                    amount: Decimal::new(4250, 2),
                    direction: Direction::Credit,
                },
            ],
        };
        assert_eq!(
            txn.fingerprint(),
            (
                NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
                "Coffee".to_string(),
                "Mogador".to_string(),
                Decimal::new(8500, 2)
            )
        );
    }

    #[test]
    fn resolve_account_prefers_exact_then_unmangled() {
        let accounts = vec![
            (uuid::Uuid::nil(), "Cash on Hand".to_string()),
            (uuid::Uuid::nil(), "Sales Revenue".to_string()),
        ];
        assert_eq!(
            resolve_account(&accounts, "Cash on Hand"),
            Some(uuid::Uuid::nil())
        );
        // Export mangling.
        assert_eq!(
            resolve_account(&accounts, "Cash_on_Hand"),
            Some(uuid::Uuid::nil())
        );
        // Beancount type root.
        assert_eq!(
            resolve_account(&accounts, "Assets:Cash on Hand"),
            Some(uuid::Uuid::nil())
        );
        // Unknown account.
        assert_eq!(resolve_account(&accounts, "Nope"), None);
    }
}
