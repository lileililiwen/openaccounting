use askama::Template;
use uuid::Uuid;

use crate::auth::User;

#[derive(Template)]
#[template(path = "admin/audit_verify.html")]
pub struct AuditVerifyPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub intact: bool,
    pub entries_count: i64,
    pub tail_hash: String,
    pub breakpoints: Vec<BreakPointRow>,
}

pub struct BreakPointRow {
    pub id: Uuid,
    pub reason: String,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboardPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub total_users: i64,
    pub total_ledgers: i64,
    pub total_transactions: i64,
    pub recent_users: Vec<RecentUserRow>,
}

#[derive(Template)]
#[template(path = "admin/users.html")]
pub struct AdminUsersPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub users: Vec<UserRow>,
}

pub struct RecentUserRow {
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at_display: String,
}

pub struct UserRow {
    pub username: String,
    pub email: String,
    pub role: String,
    pub is_active: bool,
    pub created_at_display: String,
}

impl AdminDashboardPage {
    pub fn new(
        user: User,
        total_users: i64,
        total_ledgers: i64,
        total_transactions: i64,
        recent_users: Vec<RecentUserRow>,
    ) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            total_users,
            total_ledgers,
            total_transactions,
            recent_users,
        }
    }
}

impl AdminUsersPage {
    pub fn new(user: User, users: Vec<UserRow>) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            users,
        }
    }
}
