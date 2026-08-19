use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "budgets/list.html")]
pub struct BudgetList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub budgets: Vec<BudgetRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "budgets/form.html")]
pub struct BudgetForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub period: String,
    pub amount: String,
    pub alert_threshold: String,
    pub start_date: String,
    pub end_date: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "budgets/report.html")]
pub struct BudgetReport {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub from: String,
    pub to: String,
    pub rows: Vec<BudgetVsActual>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct BudgetRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub account_name: String,
    pub period: String,
    pub amount: Decimal,
    pub alert_threshold: Decimal,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Clone, Debug)]
pub struct BudgetVsActual {
    pub budget_id: Uuid,
    pub account_name: String,
    pub budget_amount: Decimal,
    pub actual: Decimal,
    pub variance: Decimal,
    pub variance_pct: Decimal,
}
