use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
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
}
