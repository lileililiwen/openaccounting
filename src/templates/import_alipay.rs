use askama::Template;
use uuid::Uuid;

use crate::domain::Account;
use crate::handlers::import::ParsedRow;

#[derive(Template)]
#[template(path = "import/alipay_upload.html")]
pub struct AlipayUpload {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "import/alipay_preview.html")]
pub struct AlipayPreview {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub filename: String,
    /// "alipay_mobile" or "alipay_web".
    pub format: String,
    pub rows: Vec<ParsedRow>,
    /// JSON-encoded `rows`, posted back to the commit handler.
    pub rows_json: String,
    pub accounts: Vec<Account>,
    pub default_account_id: Uuid,
    pub include_other: bool,
    pub include_pending: bool,
    pub error: String,
}
