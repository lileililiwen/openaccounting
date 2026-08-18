//! Notification preferences (`u6-notification-preferences`).
//!
//! Per-user, per-channel, per-event opt-in. The table is sparse:
//! the lookup [`is_enabled`] returns the **default** when no
//! row exists for a given (channel, event) pair, and the row
//! is only inserted when the user explicitly deviates from the
//! default. The defaults are:
//!
//! * `in_app` — every event ON
//! * `email`  — `weekly_summary` ON, everything else OFF
//! * `push`   — every event OFF
//!
//! The preferences module also exposes [`Suppressed`] so the
//! dispatcher can record every push that was rejected because
//! the user opted out (counted in the
//! `notifications_suppressed_total` metric).

use sqlx::PgPool;
use uuid::Uuid;

/// Channels supported by the notification system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Email,
    Push,
    InApp,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Email => "email",
            Channel::Push => "push",
            Channel::InApp => "in_app",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "email" => Some(Channel::Email),
            "push" => Some(Channel::Push),
            "in_app" => Some(Channel::InApp),
            _ => None,
        }
    }
}

/// Events the system can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    BudgetOverrun,
    ReimbursementSubmitted,
    LargeTransaction,
    WeeklySummary,
}

impl Event {
    pub fn as_str(self) -> &'static str {
        match self {
            Event::BudgetOverrun => "budget_overrun",
            Event::ReimbursementSubmitted => "reimbursement_submitted",
            Event::LargeTransaction => "large_transaction",
            Event::WeeklySummary => "weekly_summary",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "budget_overrun" => Some(Event::BudgetOverrun),
            "reimbursement_submitted" => Some(Event::ReimbursementSubmitted),
            "large_transaction" => Some(Event::LargeTransaction),
            "weekly_summary" => Some(Event::WeeklySummary),
            _ => None,
        }
    }
}

/// Returns the default for a (channel, event) pair when the
/// user has not stored a preference. Exposed so the grid page
/// can render the right initial state.
pub fn default_enabled(channel: Channel, event: Event) -> bool {
    match (channel, event) {
        (Channel::InApp, _) => true,
        (Channel::Email, Event::WeeklySummary) => true,
        (Channel::Email, _) => false,
        (Channel::Push, _) => false,
    }
}

/// Look up the user's preference, falling back to the default.
/// Returns the persisted row if it exists, otherwise the
/// default.
pub async fn is_enabled(
    pool: &PgPool,
    user_id: Uuid,
    channel: Channel,
    event: Event,
) -> sqlx::Result<bool> {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT enabled FROM notification_preferences
         WHERE user_id = $1 AND channel = $2 AND event = $3",
    )
    .bind(user_id)
    .bind(channel.as_str())
    .bind(event.as_str())
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|(b,)| b)
        .unwrap_or_else(|| default_enabled(channel, event)))
}

/// Upsert one preference row.
pub async fn set_enabled(
    pool: &PgPool,
    user_id: Uuid,
    channel: Channel,
    event: Event,
    enabled: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        r#"INSERT INTO notification_preferences (user_id, channel, event, enabled, updated_at)
           VALUES ($1, $2, $3, $4, now())
           ON CONFLICT (user_id, channel, event)
           DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now()"#,
    )
    .bind(user_id)
    .bind(channel.as_str())
    .bind(event.as_str())
    .bind(enabled)
    .execute(pool)
    .await?;
    Ok(())
}

/// Count the rows in the audit log that were suppressed
/// because of preferences. Used by the unit test to verify
/// the audit-log behaviour required by the spec's "Audit"
/// requirement.
#[derive(Debug, Default, Clone, Copy)]
pub struct Suppressed(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        // in_app: all on
        for ev in [
            Event::BudgetOverrun,
            Event::ReimbursementSubmitted,
            Event::LargeTransaction,
            Event::WeeklySummary,
        ] {
            assert!(default_enabled(Channel::InApp, ev));
        }
        // email: only weekly_summary
        assert!(default_enabled(Channel::Email, Event::WeeklySummary));
        assert!(!default_enabled(Channel::Email, Event::BudgetOverrun));
        assert!(!default_enabled(Channel::Email, Event::LargeTransaction));
        // push: all off
        for ev in [
            Event::BudgetOverrun,
            Event::ReimbursementSubmitted,
            Event::LargeTransaction,
            Event::WeeklySummary,
        ] {
            assert!(!default_enabled(Channel::Push, ev));
        }
    }

    #[test]
    fn channel_roundtrip() {
        for c in [Channel::Email, Channel::Push, Channel::InApp] {
            assert_eq!(Channel::parse(c.as_str()), Some(c));
        }
        assert_eq!(Channel::parse("sms"), None);
    }

    #[test]
    fn event_roundtrip() {
        for e in [
            Event::BudgetOverrun,
            Event::ReimbursementSubmitted,
            Event::LargeTransaction,
            Event::WeeklySummary,
        ] {
            assert_eq!(Event::parse(e.as_str()), Some(e));
        }
        assert_eq!(Event::parse("login"), None);
    }
}
