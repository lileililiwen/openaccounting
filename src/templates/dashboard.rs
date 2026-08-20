use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub currency: String,
    pub total_assets: String,
    pub total_liabilities: String,
    pub net_worth: String,
    pub month_income: String,
    pub month_expense: String,
    pub month_net: String,
    pub txn_count: i64,
    pub recent_transactions: Vec<crate::templates::transactions::TransactionRow>,
    pub income_expense_svg: String,
    pub expense_breakdown_svg: String,
    pub cash_runway: f64,
    pub cash_runway_color: String,
    pub revenue_mom_pct: f64,
    pub expense_mom_pct: f64,
    pub net_income_mom_pct: f64,
    pub top_expenses: Vec<(String, Decimal)>,
    pub ar_outstanding: String,
    pub ar_overdue: String,
    pub ar_count: i64,
    pub ar_overdue_count: i64,
    pub ap_outstanding: String,
    pub ap_upcoming: String,
    pub ap_count: i64,
    pub ap_overdue_count: i64,
    /// Ordered list of widget IDs the user wants rendered
    /// (`u5-dashboard-widgets`). The template iterates over
    /// this and emits one `<section>` per ID in the right order.
    pub layout: Vec<String>,
    /// Per-budget row for the `budget_burn` widget.
    pub budget_burn: Vec<crate::handlers::dashboard_layout::BudgetBurnRow>,
    /// Top-10 account balances for the `account_balances`
    /// widget.
    pub account_balances: Vec<crate::handlers::dashboard_layout::AccountBalanceRow>,
    /// Setup checklist card (`a16-onboarding-quickstart`). `Some` while
    /// any milestone is pending; `None` once setup is complete.
    pub setup: Option<crate::templates::onboarding::SetupStatus>,
}
