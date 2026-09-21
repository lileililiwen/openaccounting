use askama::Template;
use chrono::NaiveDate;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "recurring/list.html")]
pub struct RecurringJournalList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub templates: Vec<TemplateRow>,
    pub runs: Vec<RunRow>,
}

#[derive(Template)]
#[template(path = "recurring/new.html")]
pub struct RecurringJournalNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String)>,
    pub cost_centers: Vec<(Uuid, String)>,
    pub projects: Vec<(Uuid, String)>,
    pub start_date: NaiveDate,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TemplateRow {
    pub id: Uuid,
    pub name: String,
    pub frequency: String,
    pub start_date: NaiveDate,
    pub next_period: NaiveDate,
    pub is_paused: bool,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RunRow {
    pub id: Uuid,
    pub template_id: Uuid,
    pub template_name: String,
    pub period_key: String,
    pub draft_txn_id: Option<Uuid>,
    pub posted_txn_id: Option<Uuid>,
    pub status: String,
}
