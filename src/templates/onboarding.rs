use askama::Template;
use uuid::Uuid;

/// One milestone on the setup checklist (`a16-onboarding-quickstart`).
#[derive(Clone, Debug)]
pub struct SetupStep {
    pub id: String,
    pub label: String,
    pub description: String,
    pub done: bool,
    pub href: String,
}

/// The full data-driven setup status for a ledger.
#[derive(Clone, Debug)]
pub struct SetupStatus {
    pub steps: Vec<SetupStep>,
    pub done: usize,
    pub total: usize,
    pub complete: bool,
}

#[derive(Template)]
#[template(path = "onboarding/setup.html")]
pub struct SetupPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub status: SetupStatus,
}
