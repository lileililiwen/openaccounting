use askama::Template;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "bank_feeds/list.html")]
pub struct BankFeedList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub links: Vec<LinkRow>,
}

#[derive(Clone, Debug)]
pub struct LinkRow {
    pub id: Uuid,
    pub provider: String,
    pub institution_id: Option<String>,
    pub account_id_at_provider: Option<String>,
    pub status: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "bank_feeds/link.html")]
pub struct BankFeedLinkPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String)>,
}
