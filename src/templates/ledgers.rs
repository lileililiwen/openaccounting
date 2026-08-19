use askama::Template;
use uuid::Uuid;

use crate::domain::Ledger;

#[derive(Template)]
#[template(path = "ledgers/list.html")]
pub struct LedgerList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub ledgers: Vec<(Uuid, String, String, String)>,
    pub pending_invitations: Vec<(Uuid, String, String)>,
    pub flash: String,
}

#[derive(Template)]
#[template(path = "ledgers/new.html")]
pub struct LedgerNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "ledgers/show.html")]
pub struct LedgerShow {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub ledger: Ledger,
    pub account_count: i64,
    pub txn_count: i64,
}
