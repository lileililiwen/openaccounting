use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "closing/page.html")]
pub struct ClosePage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub closed_through: Option<chrono::NaiveDate>,
    pub reopens: Vec<(String, String, Option<String>)>,
}

#[derive(Template)]
#[template(path = "closing/approvals.html")]
pub struct ApprovalQueue {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub pending: Vec<(Uuid, String, String, String, Uuid)>,
}

#[derive(Template)]
#[template(path = "closing/gaps.html")]
pub struct InvoiceGaps {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub year: i32,
    pub allocated: Vec<String>,
    pub voids: Vec<String>,
    pub missing: Vec<String>,
}
