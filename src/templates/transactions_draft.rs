use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "transactions/drafts.html")]
pub struct DraftsPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub drafts: Vec<DraftRow>,
    pub flash: String,
}

#[derive(Clone, Debug)]
pub struct DraftRow {
    pub id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub currency: String,
    pub payee: String,
    pub total: Decimal,
}
