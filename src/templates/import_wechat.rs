use askama::Template;
use uuid::Uuid;

use crate::domain::Account;
use crate::handlers::import::ParsedRow;

#[derive(Template)]
#[template(path = "import/wechat_upload.html")]
pub struct WechatUpload {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "import/wechat_preview.html")]
pub struct WechatPreview {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub filename: String,
    /// "wechat" — shown in the preview header.
    pub format: String,
    pub rows: Vec<ParsedRow>,
    /// JSON-encoded `rows`, posted back to the commit handler.
    pub rows_json: String,
    pub accounts: Vec<Account>,
    pub default_account_id: Uuid,
    pub include_pending: bool,
    pub error: String,
}
