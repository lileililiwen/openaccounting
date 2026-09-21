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
    /// Manual override audit trail (actor, old/new, reason).
    pub overrides: Vec<FxOverrideRow>,
    pub error: String,
}

/// One audited manual FX override for the rates page.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct FxOverrideRow {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate_date: NaiveDate,
    pub old_rate: Option<Decimal>,
    pub new_rate: Decimal,
    pub reason: String,
    pub actor_name: String,
}
