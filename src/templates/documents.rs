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
    pub current_section: String,
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
    /// `true` for documents uploaded without a transaction
    /// (`a13-document-inbox`); the inbox shows a bind action.
    pub is_unbound: bool,
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
            transaction_id: d.transaction_id.unwrap_or_default(),
            is_unbound: d.transaction_id.is_none(),
            transaction_date: date,
            transaction_description: desc,
            ocr_status: String::new(),
        }
    }
}

#[derive(Template)]
#[template(path = "documents/bind.html")]
pub struct DocumentBindPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub doc_id: Uuid,
    pub doc_filename: String,
    pub search: BindSearchState,
    pub results: Vec<BindTransactionRow>,
}

pub struct BindSearchState {
    pub q: String,
    pub from: String,
    pub to: String,
    pub amount: String,
}

pub struct BindTransactionRow {
    pub id: Uuid,
    pub date: chrono::NaiveDate,
    pub description: String,
    pub total: rust_decimal::Decimal,
}

impl DocumentBindPage {
    pub fn new(
        user: crate::auth::User,
        ledger_id: Uuid,
        ledger_name: String,
        doc_id: Uuid,
        doc_filename: String,
        search: BindSearchState,
        results: Vec<BindTransactionRow>,
    ) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id,
            ledger_name,
            current_section: "documents".to_string(),
            doc_id,
            doc_filename,
            search,
            results,
        }
    }
}
