//! Full-ledger export (`o1-ledger-export`).
//!
//! Two formats share the same in-memory snapshot:
//!
//! * [`json`] — canonical, round-trippable. Every account,
//!   transaction, posting, tag, document reference, and budget
//!   the user has access to.
//! * [`beancount`] — derived plain-text export that loads in
//!   Beancount / Fava with no further transformation.
//!
//! Both producers read under a single REPEATABLE READ
//! transaction so concurrent writes cannot tear the snapshot.

pub mod beancount;
pub mod hledger;
pub mod json;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

/// In-memory snapshot of one ledger, fully populated.
#[derive(Debug, Clone, Serialize)]
pub struct LedgerSnapshot {
    pub ledger: LedgerMeta,
    pub accounts: Vec<AccountRow>,
    pub transactions: Vec<TransactionRow>,
    pub postings: Vec<PostingRow>,
    pub tags: Vec<TagRow>,
    pub document_refs: Vec<DocumentRef>,
    pub budgets: Vec<BudgetRow>,
    /// ISO-4217 commodity codes seen across all postings, in
    /// insertion order. Used by the Beancount exporter to emit
    /// a `option "operating_currency"` directive + commodity
    /// declarations.
    pub currencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerMeta {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub timezone: String,
    pub basis: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountRow {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: Option<String>,
    pub r#type: String,
    pub subtype: String,
    pub currency: String,
    pub is_archived: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionRow {
    pub id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub currency: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub tag_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostingRow {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub direction: String,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagRow {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentRef {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub period: String,
    pub amount: Decimal,
    pub currency: String,
    pub alert_threshold: Decimal,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl LedgerSnapshot {
    /// Snapshot every row the user has access to in `ledger_id`.
    /// The transaction is opened with `SET TRANSACTION ISOLATION
    /// LEVEL REPEATABLE READ READ ONLY` so the snapshot is
    /// internally consistent.
    pub async fn load(pool: &sqlx::PgPool, ledger_id: Uuid) -> sqlx::Result<Self> {
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;

        let ledger = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                String,
                String,
                String,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, owner_id, name, base_currency, timezone, basis, created_at, updated_at
             FROM ledgers WHERE id = $1",
        )
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)?;
        let ledger = LedgerMeta {
            id: ledger.0,
            owner_id: ledger.1,
            name: ledger.2,
            base_currency: ledger.3,
            timezone: ledger.4,
            basis: ledger.5,
            created_at: ledger.6,
            updated_at: ledger.7,
        };

        let account_rows: Vec<(
            Uuid,
            Option<Uuid>,
            String,
            Option<String>,
            String,
            String,
            String,
            bool,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, parent_id, name, code, type, subtype, currency, is_archived, description
             FROM accounts WHERE ledger_id = $1 ORDER BY type, code NULLS LAST, name",
        )
        .bind(ledger_id)
        .fetch_all(&mut *tx)
        .await?;
        let accounts: Vec<AccountRow> = account_rows
            .into_iter()
            .map(|a| AccountRow {
                id: a.0,
                parent_id: a.1,
                name: a.2,
                code: a.3,
                r#type: a.4,
                subtype: a.5,
                currency: a.6,
                is_archived: a.7,
                description: a.8,
            })
            .collect();

        let txn_rows = sqlx::query_as::<
            _,
            (
                Uuid,
                NaiveDate,
                String,
                Option<String>,
                Option<String>,
                String,
                Uuid,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, txn_date, description, payee, reference, currency, created_by, created_at
             FROM transactions WHERE ledger_id = $1 ORDER BY txn_date, created_at, id",
        )
        .bind(ledger_id)
        .fetch_all(&mut *tx)
        .await?;
        let txn_ids: Vec<Uuid> = txn_rows.iter().map(|t| t.0).collect();
        let mut transactions = Vec::with_capacity(txn_rows.len());
        for t in txn_rows {
            let tag_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT tag_id FROM transaction_tags WHERE transaction_id = $1 ORDER BY tag_id",
            )
            .bind(t.0)
            .fetch_all(&mut *tx)
            .await?;
            transactions.push(TransactionRow {
                id: t.0,
                txn_date: t.1,
                description: t.2,
                payee: t.3,
                reference: t.4,
                currency: t.5,
                created_by: t.6,
                created_at: t.7,
                tag_ids,
            });
        }

        let postings = if txn_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, (Uuid, Uuid, Uuid, Decimal, String, Option<String>, String)>(
                "SELECT p.id, p.transaction_id, p.account_id, p.amount,
                        p.direction, p.memo, t.currency
                 FROM postings p JOIN transactions t ON t.id = p.transaction_id
                 WHERE p.transaction_id = ANY($1)
                 ORDER BY p.transaction_id, p.id",
            )
            .bind(&txn_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|p| PostingRow {
                id: p.0,
                transaction_id: p.1,
                account_id: p.2,
                amount: p.3,
                direction: p.4,
                memo: p.5,
                currency: p.6,
            })
            .collect()
        };

        let tags: Vec<TagRow> = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM tags WHERE ledger_id = $1 ORDER BY name",
        )
        .bind(ledger_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(id, name)| TagRow { id, name })
        .collect();

        let document_refs: Vec<DocumentRef> = sqlx::query_as::<
            _,
            (Uuid, Uuid, String, String, i64, DateTime<Utc>),
        >(
            "SELECT d.id, d.transaction_id, d.filename, d.mime_type, d.size_bytes, d.uploaded_at
             FROM documents d JOIN transactions t ON t.id = d.transaction_id
             WHERE t.ledger_id = $1 ORDER BY d.uploaded_at, d.id",
        )
        .bind(ledger_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|d| DocumentRef {
            id: d.0,
            transaction_id: d.1,
            filename: d.2,
            mime_type: d.3,
            size_bytes: d.4,
            uploaded_at: d.5,
        })
        .collect();

        // budgets.currency is derived from the account's currency.
        let budgets: Vec<BudgetRow> = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                Decimal,
                Decimal,
                NaiveDate,
                NaiveDate,
                String,
            ),
        >(
            "SELECT b.id, b.account_id, b.period, b.amount, b.alert_threshold,
                    b.start_date, b.end_date, a.currency
             FROM budgets b JOIN accounts a ON a.id = b.account_id
             WHERE b.ledger_id = $1 ORDER BY b.start_date, b.id",
        )
        .bind(ledger_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|b| BudgetRow {
            id: b.0,
            account_id: b.1,
            period: b.2,
            amount: b.3,
            currency: b.7,
            alert_threshold: b.4,
            start_date: b.5,
            end_date: b.6,
        })
        .collect();

        let mut currencies: Vec<String> = Vec::new();
        for cur in accounts.iter().map(|a| a.currency.as_str()) {
            if !currencies.iter().any(|c| c == cur) {
                currencies.push(cur.to_string());
            }
        }
        for cur in postings.iter().map(|p| p.currency.as_str()) {
            if !currencies.iter().any(|c| c == cur) {
                currencies.push(cur.to_string());
            }
        }
        if !currencies.iter().any(|c| c == &ledger.base_currency) {
            currencies.push(ledger.base_currency.clone());
        }

        tx.commit().await?;

        Ok(Self {
            ledger,
            accounts,
            transactions,
            postings,
            tags,
            document_refs,
            budgets,
            currencies,
        })
    }
}
