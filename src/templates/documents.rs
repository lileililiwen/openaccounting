use askama::Template;
use uuid::Uuid;

use crate::domain::Document;

#[derive(Template)]
#[template(path = "documents/list.html")]
pub struct DocumentList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub documents: Vec<DocumentWithTxn>,
}

#[derive(Clone, Debug)]
pub struct DocumentWithTxn {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
    pub transaction_id: Uuid,
    pub transaction_date: chrono::NaiveDate,
    pub transaction_description: String,
    /// One of: `""`, `"pending"`, `"done"`, `"failed"`.
    pub ocr_status: String,
}

impl From<(Document, chrono::NaiveDate, String)> for DocumentWithTxn {
    fn from((d, date, desc): (Document, chrono::NaiveDate, String)) -> Self {
        Self {
            id: d.id,
            filename: d.filename,
            mime_type: d.mime_type,
            size_bytes: d.size_bytes,
            uploaded_at: d.uploaded_at,
            transaction_id: d.transaction_id,
            transaction_date: date,
            transaction_description: desc,
            ocr_status: String::new(),
        }
    }
}
