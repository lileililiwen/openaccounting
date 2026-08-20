use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "transfers/inter_ledger.html")]
pub struct TransfersPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    /// `(ledger_id, ledger_name, [(account_id, account_name)])` for
    /// every ledger the user can write to.
    pub ledgers: Vec<(Uuid, String, Vec<(Uuid, String)>)>,
    pub error: String,
}
