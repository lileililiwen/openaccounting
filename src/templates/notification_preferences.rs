use askama::Template;
use uuid::Uuid;

use crate::notifications::preferences::Event;

#[derive(Template)]
#[template(path = "account/notifications.html")]
pub struct PreferencesPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    /// Empty when the user has not opened a ledger. Required by
    /// the `_nav.html` partial which conditionally renders the
    /// ledger-scoped links.
    pub ledger_id: Uuid,
    pub ledger_name: String,
    /// One row per event, in display order.
    pub rows: Vec<PreferencesRow>,
}

pub struct PreferencesRow {
    pub event_id: &'static str,
    pub event_label: &'static str,
    pub in_app: bool,
    pub email: bool,
    pub push: bool,
}

impl PreferencesRow {
    pub const EVENTS: [Event; 4] = [
        Event::BudgetOverrun,
        Event::ReimbursementSubmitted,
        Event::LargeTransaction,
        Event::WeeklySummary,
    ];
}

impl PreferencesPage {
    pub fn build(
        user_id: Uuid,
        username: String,
        user_role: String,
        // 3 booleans per event, in (in_app, email, push) order.
        cells: Vec<bool>,
    ) -> Self {
        let rows = PreferencesRow::EVENTS
            .iter()
            .enumerate()
            .map(|(i, ev)| PreferencesRow {
                event_id: ev.as_str(),
                event_label: match ev {
                    Event::BudgetOverrun => "Budget overrun",
                    Event::ReimbursementSubmitted => "Reimbursement submitted",
                    Event::LargeTransaction => "Large transaction",
                    Event::WeeklySummary => "Weekly summary",
                },
                in_app: cells[i * 3],
                email: cells[i * 3 + 1],
                push: cells[i * 3 + 2],
            })
            .collect();
        Self {
            user_id,
            username,
            user_role,
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            rows,
        }
    }
}
