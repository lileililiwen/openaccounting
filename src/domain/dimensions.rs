//! Posting dimensions (`accounting-dimensions`).
//!
//! Cost centers and projects are ledger-scoped dimensions carried
//! per posting (nullable — existing postings stay valid). The balance
//! invariant is untouched: dimensions never change amounts, only slice
//! reports. Pure helpers here; SQL lives in the posting service and
//! the report builders.

use uuid::Uuid;

/// Dimension slice accepted by trial balance and P&L.
#[derive(Clone, Debug, Default)]
pub struct DimensionFilter {
    pub cost_center_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

impl DimensionFilter {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.cost_center_id.is_some() || self.project_id.is_some()
    }
}

/// Parse an optional UUID query param. Empty/missing → None; garbage →
/// `Err` naming the parameter (400 at the handler).
pub fn parse_optional_id(raw: Option<&str>, param: &str) -> Result<Option<Uuid>, String> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => Uuid::parse_str(s)
            .map(Some)
            .map_err(|_| format!("invalid {param} '{s}', expected a UUID")),
    }
}
