use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub currency: String,
    pub kind: String,
    pub contact_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A transaction line as the user sees it on the form. At least two legs are
/// required, and the absolute values of debits must equal those of credits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxnLineInput {
    pub account_id: Uuid,
    /// Signed amount. Positive = debit, negative = credit. Server normalizes
    /// into separate (amount, direction) on insert.
    pub signed_amount: Decimal,
    pub memo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTransaction {
    pub ledger_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub currency: String,
    pub lines: Vec<TxnLineInput>,
}
