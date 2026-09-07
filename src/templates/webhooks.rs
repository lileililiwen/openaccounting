use askama::Template;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: Uuid,
    pub target_url: String,
    pub events: Vec<String>,
    pub is_enabled: bool,
    pub failed_count: i64,
}

#[derive(Template)]
#[template(path = "webhooks/list.html")]
pub struct WebhookList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub subscriptions: Vec<WebhookRow>,
    pub event_types: Vec<String>,
    /// Shown exactly once after create/rotate; empty otherwise.
    pub new_secret: String,
    pub error: String,
}
