use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "fx/list.html")]
pub struct FxRatesPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub base_currency: String,
    pub rates: Vec<(String, String, Decimal, NaiveDate, String)>,
    pub error: String,
}
