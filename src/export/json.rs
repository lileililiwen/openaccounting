//! JSON exporter for [`LedgerSnapshot`]. The format is the
//! canonical one: re-importing the same document must produce
//! rows that compare equal to the source.
//!
//! Both the field order inside each row and the order of rows
//! inside the document are stable because every SELECT in
//! `LedgerSnapshot::load` uses an explicit `ORDER BY`.

use serde::Serialize;

use super::LedgerSnapshot;

/// Wrapper struct that adds the format version + export
/// timestamp so future schema changes can be detected by
/// re-importers.
#[derive(Debug, Serialize)]
pub struct LedgerEnvelope<'a> {
    pub format: &'static str,
    pub format_version: u32,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub snapshot: &'a LedgerSnapshot,
}

pub const FORMAT: &str = "openaccounting-ledger";
pub const FORMAT_VERSION: u32 = 1;

impl<'a> LedgerEnvelope<'a> {
    pub fn new(snapshot: &'a LedgerSnapshot) -> Self {
        Self {
            format: FORMAT,
            format_version: FORMAT_VERSION,
            exported_at: chrono::Utc::now(),
            snapshot,
        }
    }
}

/// Serialise the snapshot as a stable JSON document. The
/// returned value is owned `serde_json::Value` so callers can
/// pretty-print or stream it themselves.
pub fn to_value(snapshot: &LedgerSnapshot) -> serde_json::Value {
    serde_json::to_value(LedgerEnvelope::new(snapshot)).expect("snapshot is always serializable")
}

/// Serialise the snapshot as a UTF-8 JSON string with
/// human-readable indentation. Suitable for download endpoints
/// that want diff-friendly output.
pub fn to_string_pretty(snapshot: &LedgerSnapshot) -> String {
    serde_json::to_string_pretty(&LedgerEnvelope::new(snapshot))
        .expect("snapshot is always serializable")
}
