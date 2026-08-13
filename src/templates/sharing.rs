use askama::Template;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "sharing/page.html")]
pub struct SharePage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub members: Vec<(Uuid, String, String, DateTime<Utc>)>,
    pub invitations: Vec<(String, String, DateTime<Utc>)>,
    pub error: String,
}
