use askama::Template;
use uuid::Uuid;

use crate::audit::AuditEntry;

#[derive(Template)]
#[template(path = "audit/list.html")]
pub struct AuditLogPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub entries: Vec<(AuditEntry, String)>,
    pub offset: i64,
    pub limit: i64,
}
