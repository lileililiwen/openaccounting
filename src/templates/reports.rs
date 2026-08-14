use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::handlers::aging::AgingBucketData;
use crate::reports::cash_flow::CashFlowLine;
use crate::reports::income_statement::ExcludedTotals;
use crate::reports::ReportBasis;

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
    pub assets: Vec<BalanceSheetSection>,
    pub liabilities: Vec<BalanceSheetSection>,
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
    pub basis: ReportBasis,
    pub revenue: IncomeStatementSection,
    pub cost_of_goods_sold: IncomeStatementSection,
    pub gross_profit: Decimal,
    pub operating_expenses: IncomeStatementSection,
    pub operating_income: Decimal,
    pub non_operating: Option<IncomeStatementSection>,
    pub income_before_tax: Decimal,
    pub tax_expense: Option<IncomeStatementSection>,
    pub net_income: Decimal,
    pub excluded: Option<ExcludedTotals>,
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
    pub basis: ReportBasis,
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

#[derive(Template)]
#[template(path = "reports/aging.html")]
pub struct AgingReportPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub report_type: String,
    pub as_of: NaiveDate,
    pub aging: Vec<AgingBucketData>,
}
