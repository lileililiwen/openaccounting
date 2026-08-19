use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::reimbursement::Claim;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ClaimListRow {
    pub id: Uuid,
    pub short_id: String,
    pub title: String,
    pub employee_name: String,
    pub status: String,
    pub currency: String,
    pub total: Decimal,
}

#[derive(Template)]
#[template(path = "reimbursements/list.html")]
pub struct ClaimList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub claims: Vec<ClaimListRow>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "reimbursements/new.html")]
pub struct ClaimNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub error: String,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ClaimShowLine {
    pub id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub gl_account_id: Uuid,
    pub tax_amount: Decimal,
    pub advance_amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct ApprovalLevelView {
    pub level: i32,
    pub status: String,
    pub approver_name: String,
    pub approved_at: String,
}

#[derive(Template)]
#[template(path = "reimbursements/show.html")]
pub struct ClaimShow {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub claim: Claim,
    pub lines: Vec<ClaimShowLine>,
    pub total: Decimal,
    /// Whether the current user is the claim's author. Authors only
    /// see the aggregate approval status.
    pub viewer_is_author: bool,
    /// Number of required approval levels not yet recorded (author view).
    pub pending_count: usize,
    /// Levels the current user may approve right now (eligible and
    /// not yet recorded).
    pub approve_levels: Vec<i32>,
    /// Per-level status for approvers.
    pub approval_levels: Vec<ApprovalLevelView>,
    pub error: String,
}
