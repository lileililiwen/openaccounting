use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Invoice {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub contact_id: Uuid,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total: Decimal,
    pub amount_paid: Decimal,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewInvoice {
    pub contact_id: Uuid,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total: Decimal,
}
