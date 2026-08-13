use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "templates/list.html")]
pub struct TemplateList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub templates: Vec<TemplateRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "templates/new.html")]
pub struct TemplateNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub description: String,
    pub payee: String,
    pub reference: String,
    pub frequency: String,
    pub next_date: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "templates/show.html")]
pub struct TemplateShow {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub template: TemplateRow,
    pub postings: Vec<(Uuid, String, String, String, Decimal, Option<String>)>,
}

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct TemplateRow {
    pub id: Uuid,
    pub description: String,
    pub payee: String,
    pub frequency: String,
    pub next_date: NaiveDate,
    pub is_active: bool,
    pub posting_count: i64,
}
