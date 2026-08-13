use askama::Template;
use uuid::Uuid;

use crate::domain::Ledger;

#[derive(Template)]
#[template(path = "ledgers/list.html")]
pub struct LedgerList {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub ledgers: Vec<Ledger>,
    pub flash: String,
}

#[derive(Template)]
#[template(path = "ledgers/new.html")]
pub struct LedgerNew {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "ledgers/show.html")]
pub struct LedgerShow {
    pub user_id: Uuid,
    pub username: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub ledger: Ledger,
    pub account_count: i64,
    pub txn_count: i64,
}
