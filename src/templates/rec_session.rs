use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "rec_session/page.html")]
pub struct RecSessionPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub account_id: Uuid,
    pub session_id: Uuid,
    pub stmt_close_date: NaiveDate,
    pub stmt_close_balance: Decimal,
    pub opening_balance: Decimal,
    pub cleared_sum: Decimal,
    pub difference: Decimal,
    pub status: String,
    pub locked: bool,
    pub lines: Vec<RecSessionLine>,
    pub flash: String,
}

#[derive(Clone, Debug)]
pub struct RecSessionLine {
    pub id: Uuid,
    pub statement_date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub cleared: bool,
    pub suggested_rule: Option<String>,
}
