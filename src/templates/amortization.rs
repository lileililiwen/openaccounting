use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "amortization/report.html")]
pub struct AmortizationReportPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub schedules: Vec<AmortizationRowTemplate>,
}

#[derive(Clone, Debug)]
pub struct AmortizationRowTemplate {
    pub schedule_id: Uuid,
    pub description: String,
    pub source_account_name: String,
    pub target_account_name: String,
    pub total_amount: Decimal,
    pub posted_amount: Decimal,
    pub remaining_amount: Decimal,
    pub period_unit: String,
    pub periods: i32,
    pub posted_periods: i32,
    pub skipped_periods: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub next_post_date: NaiveDate,
}

#[derive(Template)]
#[template(path = "amortization/new.html")]
pub struct AmortizationNewPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub error: String,
}
