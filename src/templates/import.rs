use askama::Template;
use uuid::Uuid;

use crate::domain::Account;

use crate::handlers::import::{CsvMapping, ParsedRow};

#[derive(Template)]
#[template(path = "import/upload.html")]
pub struct ImportUpload {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "import/preview.html")]
pub struct ImportPreview {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub filename: String,
    /// "csv", "ofx", "qif", or "mt940". Shown in the preview
    /// header so the user knows which parser produced the
    /// rows.
    pub format: String,
    pub headers: Vec<String>,
    pub rows: Vec<ParsedRow>,
    pub accounts: Vec<Account>,
    pub mapping: CsvMapping,
    pub error: String,
}
