use askama::Template;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "backups/list.html")]
pub struct BackupList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub backups: Vec<(Uuid, String, i64, String, DateTime<Utc>)>,
}

#[derive(Template)]
#[template(path = "backups/integrity.html")]
pub struct IntegrityReport {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub passed: bool,
    pub txn_count: i64,
    pub posting_count: i64,
    pub account_count: i64,
    pub issues: Vec<IntegrityIssue>,
}

#[derive(Clone, Debug)]
pub struct IntegrityIssue {
    pub severity: String,
    pub check: String,
    pub message: String,
}
