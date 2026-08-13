use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::reports::cash_flow::CashFlowLine;

use crate::reports::*;

#[derive(Template)]
#[template(path = "reports/index.html")]
pub struct ReportsIndex {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
}

#[derive(Template)]
#[template(path = "reports/trial_balance.html")]
pub struct TrialBalancePage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub as_of: chrono::NaiveDate,
    pub rows: Vec<TrialBalanceRow>,
    pub totals_debit: Decimal,
    pub totals_credit: Decimal,
    pub balanced: bool,
}

#[derive(Template)]
#[template(path = "reports/balance_sheet.html")]
pub struct BalanceSheetPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub as_of: chrono::NaiveDate,
    pub assets: Vec<AccountTotal>,
    pub liabilities: Vec<AccountTotal>,
    pub equity: Vec<AccountTotal>,
    pub net_income: Decimal,
    pub total_assets: Decimal,
    pub total_liab_equity: Decimal,
    pub balanced: bool,
}

#[derive(Template)]
#[template(path = "reports/income_statement.html")]
pub struct IncomeStatementPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub from: chrono::NaiveDate,
    pub to: chrono::NaiveDate,
    pub income: Vec<AccountTotal>,
    pub expense: Vec<AccountTotal>,
    pub total_income: Decimal,
    pub total_expense: Decimal,
    pub net_income: Decimal,
}

#[derive(Template)]
#[template(path = "reports/cash_flow.html")]
pub struct CashFlowPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub from: chrono::NaiveDate,
    pub to: chrono::NaiveDate,
    pub opening: Decimal,
    pub closing: Decimal,
    pub movement: Decimal,
    pub inflows: Vec<CashFlowLine>,
    pub outflows: Vec<CashFlowLine>,
    pub total_inflows: Decimal,
    pub total_outflows: Decimal,
}

#[derive(Template)]
#[template(path = "reports/general_ledger.html")]
pub struct GeneralLedgerPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub from: chrono::NaiveDate,
    pub to: chrono::NaiveDate,
    pub account_filter: String,
    pub accounts: Vec<crate::domain::Account>,
    pub entries: Vec<GeneralLedgerEntry>,
    pub running_balances: std::collections::HashMap<Uuid, Decimal>,
}
