use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "taxes/list.html")]
pub struct TaxList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub rates: Vec<TaxRateRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "taxes/form.html")]
pub struct TaxForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub name: String,
    pub rate: String,
    pub kind: String,
    pub account_id: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "taxes/report.html")]
pub struct TaxReport {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub from: String,
    pub to: String,
    pub summary: Vec<(Uuid, String, String, Decimal, Decimal, i64)>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TaxRateRow {
    pub id: Uuid,
    pub name: String,
    pub rate: Decimal,
    pub kind: String,
    pub account_id: Uuid,
    pub is_active: bool,
}
