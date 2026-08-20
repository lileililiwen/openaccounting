use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    /// `None` for unbound documents uploaded via the ledger-level
    /// inbox (`a13-document-inbox`); set once the document is bound
    /// to a transaction.
    pub transaction_id: Option<Uuid>,
    /// Authorization anchor. `Some` for every document: bound
    /// documents carry their transaction's ledger, unbound documents
    /// carry the ledger they were uploaded to.
    pub ledger_id: Option<Uuid>,
    pub filename: String,
    pub stored_filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: Uuid,
    pub uploaded_at: DateTime<Utc>,
    pub category: String,
}
