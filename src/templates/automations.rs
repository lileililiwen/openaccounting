use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "automations/list.html")]
pub struct AutomationList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub rows: Vec<AutomationRow>,
    pub subscriptions: Vec<(Uuid, String)>,
    pub has_secret: bool,
    pub triggers: Vec<&'static str>,
    pub actions: Vec<&'static str>,
    pub error: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AutomationRow {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub conditions: String,
    pub action: String,
    pub action_config: String,
    pub is_enabled: bool,
}
