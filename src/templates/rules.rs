use askama::Template;
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct RuleRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub priority: i32,
    pub is_active: bool,
}

#[derive(Template)]
#[template(path = "rules/list.html")]
pub struct RuleList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub rules: Vec<RuleRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "rules/new.html")]
pub struct RuleNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}
