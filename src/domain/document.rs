use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub filename: String,
    pub stored_filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: Uuid,
    pub uploaded_at: DateTime<Utc>,
    pub category: String,
}
