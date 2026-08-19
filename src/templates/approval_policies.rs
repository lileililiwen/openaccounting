use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct PolicyRow {
    pub id: Uuid,
    pub name: String,
    pub min_amount: Decimal,
    pub approver_role: String,
    pub level: i32,
}

#[derive(Template)]
#[template(path = "approval_policies/list.html")]
pub struct PolicyList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub policies: Vec<PolicyRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "approval_policies/new.html")]
pub struct PolicyNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}
