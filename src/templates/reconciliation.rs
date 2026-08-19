use askama::Template;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "reconciliation/page.html")]
pub struct ReconPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub account_id: Uuid,
    pub unmatched_lines: Vec<ReconStatementLine>,
    pub candidate_txns: Vec<ReconTxn>,
    pub statement_balance: Decimal,
    pub ledger_balance: Decimal,
    pub difference: Decimal,
    pub flash: String,
}

#[derive(Template)]
#[template(path = "reconciliation/history.html")]
pub struct ReconHistory {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub account_id: Uuid,
    pub history: Vec<(NaiveDate, Decimal, Decimal, Decimal, DateTime<Utc>)>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ReconStatementLine {
    pub id: Uuid,
    pub statement_date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub check_number: String,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ReconTxn {
    pub id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: String,
    pub amount: Decimal,
}
