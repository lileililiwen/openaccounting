//! Expense reimbursement domain types and state machine.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
    Paid,
}

impl ClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimStatus::Draft => "draft",
            ClaimStatus::Submitted => "submitted",
            ClaimStatus::Approved => "approved",
            ClaimStatus::Rejected => "rejected",
            ClaimStatus::Paid => "paid",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "submitted" => Some(Self::Submitted),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "paid" => Some(Self::Paid),
            _ => None,
        }
    }
    /// Returns true iff the transition is allowed by the
    /// documented state machine:
    ///
    /// draft     → submitted
    /// submitted → approved | rejected
    /// approved  → paid
    /// rejected  → draft  (back to drafting a fix)
    /// paid      → (terminal)
    pub fn can_transition_to(self, target: ClaimStatus) -> bool {
        use ClaimStatus::*;
        match (self, target) {
            (Draft, Submitted) => true,
            (Submitted, Approved) => true,
            (Submitted, Rejected) => true,
            (Approved, Paid) => true,
            (Rejected, Draft) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Claim {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub short_id: String,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub title: String,
    pub description: Option<String>,
    pub currency: String,
    pub status: ClaimStatus,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_reason: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub payout_account_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub gl_account_id: Uuid,
    pub tax_amount: Decimal,
    pub advance_amount: Decimal,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub actor_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

/// Short, human-friendly claim id (8 hex chars from a UUID).
pub fn format_short_id(uuid: &Uuid) -> String {
    let hex = hex::encode(uuid.as_bytes());
    hex[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_allows_document_transitions() {
        use ClaimStatus::*;
        assert!(Draft.can_transition_to(Submitted));
        assert!(Submitted.can_transition_to(Approved));
        assert!(Submitted.can_transition_to(Rejected));
        assert!(Approved.can_transition_to(Paid));
        assert!(Rejected.can_transition_to(Draft));
    }

    #[test]
    fn state_machine_rejects_illegal_transitions() {
        use ClaimStatus::*;
        assert!(!Draft.can_transition_to(Approved));
        assert!(!Draft.can_transition_to(Paid));
        assert!(!Submitted.can_transition_to(Paid));
        assert!(!Approved.can_transition_to(Draft));
        assert!(!Approved.can_transition_to(Submitted));
        assert!(!Paid.can_transition_to(Draft));
        assert!(!Paid.can_transition_to(Approved));
    }

    #[test]
    fn short_id_is_stable() {
        let id = Uuid::nil();
        assert_eq!(format_short_id(&id), "00000000");
        let id2 = Uuid::nil();
        assert_eq!(format_short_id(&id), format_short_id(&id2));
    }
}
