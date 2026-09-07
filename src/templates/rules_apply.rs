use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "rules/apply_result.html")]
pub struct ApplyResult {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub changed: i64,
    pub selected: i64,
}
