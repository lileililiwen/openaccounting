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
    pub current_section: String,
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
    pub current_section: String,
    pub total_users: i64,
    pub total_ledgers: i64,
    pub total_transactions: i64,
    pub inactive_users: i64,
    pub total_documents: i64,
    pub activity_24h: i64,
    pub recent_users: Vec<RecentUserRow>,
    pub recent_activity: Vec<AuditLogRow>,
}

#[derive(Template)]
#[template(path = "admin/users.html")]
pub struct AdminUsersPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub users: Vec<UserRow>,
}

pub struct RecentUserRow {
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at_display: String,
}

pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub is_active: bool,
    /// `true` for the currently signed-in admin — the UI disables
    /// self-suspend / self-demote controls for that row
    /// (`a11-admin-console`).
    pub is_self: bool,
    pub created_at_display: String,
}

#[derive(Template)]
#[template(path = "admin/users_detail.html")]
pub struct AdminUserDetailPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub target: UserDetailHeader,
    pub ledgers: Vec<UserLedgerRow>,
    pub activity: Vec<AuditActivityRow>,
}

pub struct UserDetailHeader {
    pub username: String,
    pub email: String,
    pub role: String,
    pub is_active: bool,
    pub joined_display: String,
}

pub struct UserLedgerRow {
    pub id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub created_display: String,
}

pub struct AuditActivityRow {
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub ledger_name: String,
    pub summary: String,
    pub created_display: String,
}

#[derive(Template)]
#[template(path = "admin/audit.html")]
pub struct AdminAuditPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub users: Vec<AuditFilterUser>,
    pub filters: AuditFilters,
    pub rows: Vec<AuditLogRow>,
    pub page: i64,
    pub has_more: bool,
}

pub struct AuditFilterUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
}

pub struct AuditFilters {
    pub actor_id: String,
    pub action: String,
    pub entity: String,
    pub from: String,
    pub to: String,
}

pub struct AuditLogRow {
    pub created_display: String,
    pub actor_username: String,
    pub ledger_name: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub summary: String,
}

impl AdminDashboardPage {
    pub fn new(
        user: User,
        total_users: i64,
        total_ledgers: i64,
        total_transactions: i64,
        inactive_users: i64,
        total_documents: i64,
        activity_24h: i64,
        recent_users: Vec<RecentUserRow>,
        recent_activity: Vec<AuditLogRow>,
    ) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: "admin".to_string(),
            total_users,
            total_ledgers,
            total_transactions,
            inactive_users,
            total_documents,
            activity_24h,
            recent_users,
            recent_activity,
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
            current_section: "admin".to_string(),
            users,
        }
    }
}

impl AdminUserDetailPage {
    pub fn new(
        user: User,
        target: UserDetailHeader,
        ledgers: Vec<UserLedgerRow>,
        activity: Vec<AuditActivityRow>,
    ) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: "admin".to_string(),
            target,
            ledgers,
            activity,
        }
    }
}

impl AdminAuditPage {
    pub fn new(
        user: User,
        users: Vec<AuditFilterUser>,
        filters: AuditFilters,
        rows: Vec<AuditLogRow>,
        page: i64,
        has_more: bool,
    ) -> Self {
        Self {
            username: user.username,
            user_role: user.role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: "admin".to_string(),
            users,
            filters,
            rows,
            page,
            has_more,
        }
    }
}
