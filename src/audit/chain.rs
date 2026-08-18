//! Hash-chain for the audit log (`d1-audit-chain`).
//!
//! Every `audit_entries` row carries `prev_hash` (the previous row's
//! hash) and `hash`:
//!
//! ```text
//! hash = SHA256(prev_hash || canonical_row_bytes)
//! ```
//!
//! where `canonical_row_bytes` is a deterministic serialization of the
//! row's immutable content (ledger/actor/entity ids, action, entity
//! type, JSON diff, and `created_at`). The row's own `id` and
//! `prev_hash` are excluded so verification can recompute the hash
//! from stored columns alone.
//!
//! Appends are serialized with `pg_advisory_xact_lock` so two
//! concurrent `audit::log` calls can never read the same tail hash.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Length of a chain hash (SHA256).
pub const HASH_LEN: usize = 32;

/// A row as walked by [`verify`], carrying enough to recompute the hash.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ChainRow {
    pub id: Uuid,
    pub prev_hash: Option<Vec<u8>>,
    pub hash: Option<Vec<u8>>,
    pub ledger_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub created_at: DateTime<Utc>,
}

/// A broken link found during [`verify`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakPoint {
    /// The audit row whose hash / link is invalid.
    pub id: Uuid,
    /// Human-readable reason.
    pub reason: String,
}

/// Deterministic serialization of an audit row's content.
///
/// Layout (little-endian):
/// - optional `ledger_id`:  1 byte present flag + 16 bytes
/// - `actor_id`:            16 bytes
/// - `action`:              8-byte length + UTF-8 bytes
/// - `entity_type`:         8-byte length + UTF-8 bytes
/// - optional `entity_id`:  1 byte present flag + 16 bytes
/// - `old_value`:           1 byte NULL flag + 8-byte length + JSON bytes
/// - `new_value`:           1 byte NULL flag + 8-byte length + JSON bytes
/// - `created_at`:          8-byte micros since Unix epoch
#[allow(clippy::too_many_arguments)]
fn canonical_bytes(
    ledger_id: Option<Uuid>,
    actor_id: Uuid,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<&Value>,
    new_value: Option<&Value>,
    created_at: DateTime<Utc>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    push_opt_uuid(&mut out, ledger_id);
    out.extend_from_slice(actor_id.as_bytes());
    push_str(&mut out, action);
    push_str(&mut out, entity_type);
    push_opt_uuid(&mut out, entity_id);
    push_opt_json(&mut out, old_value);
    push_opt_json(&mut out, new_value);
    out.extend_from_slice(&created_at.timestamp_micros().to_le_bytes());
    out
}

fn push_opt_uuid(out: &mut Vec<u8>, v: Option<Uuid>) {
    match v {
        Some(u) => {
            out.push(0x01);
            out.extend_from_slice(u.as_bytes());
        }
        None => out.push(0x00),
    }
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn push_opt_json(out: &mut Vec<u8>, v: Option<&Value>) {
    match v {
        Some(j) => {
            out.push(0x01);
            // serde_json canonicalizes maps to the order produced by
            // the serializer; recomputing with the same value yields
            // the same bytes, which is all verification needs.
            let bytes = serde_json::to_vec(j).expect("serialize jsonb value");
            out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        None => out.push(0x00),
    }
}

/// Compute `SHA256(prev_hash || canonical_row_bytes)`.
pub fn compute_hash(prev_hash: Option<&[u8]>, content: &[u8]) -> [u8; HASH_LEN] {
    let mut hasher = Sha256::new();
    if let Some(prev) = prev_hash {
        hasher.update(prev);
    }
    hasher.update(content);
    hasher.finalize().into()
}

/// Append one hashed audit row. Serialized against concurrent appends
/// via a table-level advisory lock so the tail hash is always read
/// while no other append is in flight.
///
/// `created_at` is supplied by the caller so it is part of the hash;
/// the application always passes `Utc::now()`.
#[allow(clippy::too_many_arguments)]
pub async fn log_hashed(
    pool: &PgPool,
    ledger_id: Option<Uuid>,
    actor_id: Uuid,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
    created_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Serialize appends: the advisory lock key is stable for the
    // `audit_entries` table across all connections.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('audit_entries'))")
        .execute(&mut *tx)
        .await?;

    let tail_hash: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM audit_entries
         WHERE hash IS NOT NULL
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;

    // The canonical content of THIS row has to be computed before
    // insert; the tail hash becomes `prev_hash`.
    let content = canonical_bytes(
        ledger_id,
        actor_id,
        action,
        entity_type,
        entity_id,
        old_value.as_ref(),
        new_value.as_ref(),
        created_at,
    );
    let prev_hash = tail_hash.map(|(h,)| h);
    let hash = compute_hash(prev_hash.as_deref(), &content).to_vec();

    sqlx::query(
        r#"INSERT INTO audit_entries
             (ledger_id, actor_id, action, entity_type, entity_id,
              old_value, new_value, created_at, prev_hash, hash)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(ledger_id)
    .bind(actor_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_value)
    .bind(new_value)
    .bind(created_at)
    .bind(prev_hash)
    .bind(hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Backfill any rows whose `hash` is still NULL, chaining them in
/// insertion order. Idempotent; safe to call at every startup.
pub async fn ensure_backfilled(pool: &PgPool) -> Result<usize, sqlx::Error> {
    let rows: Vec<ChainRow> = sqlx::query_as(
        r#"SELECT id, prev_hash, hash, ledger_id, actor_id, action,
                  entity_type, entity_id, old_value, new_value, created_at
           FROM audit_entries
           WHERE hash IS NULL
           ORDER BY created_at ASC, id ASC"#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;
    // Rows ordered by created_at/id; the chain must continue from the
    // current tail. Fetch the last hashed row to start from it.
    let tail: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM audit_entries
         WHERE hash IS NOT NULL
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let mut prev = tail.map(|(h,)| h);

    for row in &rows {
        let content = canonical_bytes(
            row.ledger_id,
            row.actor_id,
            &row.action,
            &row.entity_type,
            row.entity_id,
            row.old_value.as_ref(),
            row.new_value.as_ref(),
            row.created_at,
        );
        let hash = compute_hash(prev.as_deref(), &content).to_vec();
        sqlx::query("UPDATE audit_entries SET prev_hash = $1, hash = $2 WHERE id = $3")
            .bind(&prev)
            .bind(&hash)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        prev = Some(hash);
    }
    tx.commit().await?;
    Ok(rows.len())
}

/// Walk the whole chain and report every broken link. Returns `Ok`
/// with an empty list when the chain is intact.
pub async fn verify(pool: &PgPool) -> Result<Vec<BreakPoint>, sqlx::Error> {
    let rows: Vec<ChainRow> = sqlx::query_as(
        r#"SELECT id, prev_hash, hash, ledger_id, actor_id, action,
                  entity_type, entity_id, old_value, new_value, created_at
           FROM audit_entries
           ORDER BY created_at ASC, id ASC"#,
    )
    .fetch_all(pool)
    .await?;

    let mut breaks = Vec::new();
    let mut prev_hash: Option<Vec<u8>> = None;
    for row in &rows {
        let Some(expected_prev) = prev_hash.as_ref() else {
            // First row: prev_hash must be empty/NULL.
            if row.prev_hash.is_some() {
                breaks.push(BreakPoint {
                    id: row.id,
                    reason: "first row carries a non-NULL prev_hash".into(),
                });
            }
            prev_hash = row.hash.clone();
            continue;
        };
        // Link check: this row's prev_hash must equal the last hash.
        if row.prev_hash.as_deref() != Some(expected_prev.as_slice()) {
            breaks.push(BreakPoint {
                id: row.id,
                reason: "prev_hash does not match the previous row's hash".into(),
            });
        }
        // Content check: recompute and compare.
        let content = canonical_bytes(
            row.ledger_id,
            row.actor_id,
            &row.action,
            &row.entity_type,
            row.entity_id,
            row.old_value.as_ref(),
            row.new_value.as_ref(),
            row.created_at,
        );
        let expected = compute_hash(row.prev_hash.as_deref(), &content).to_vec();
        if row.hash.as_deref() != Some(expected.as_slice()) {
            breaks.push(BreakPoint {
                id: row.id,
                reason: "hash does not match recomputed SHA256 of row content".into(),
            });
        }
        prev_hash = row.hash.clone();
    }
    Ok(breaks)
}

/// The hex tail hash, or `None` when the audit log is empty.
pub async fn latest_hash_hex(pool: &PgPool) -> Result<Option<String>, sqlx::Error> {
    let tail: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT hash FROM audit_entries
         WHERE hash IS NOT NULL
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(tail.map(|(h,)| hex::encode(h)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};
    use serde_json::json;

    fn sample_created() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 18, 12, 0, 0)
            .unwrap()
            .with_nanosecond(123_456_000)
            .unwrap()
    }

    #[test]
    fn hash_changes_with_prev_hash() {
        let content = b"payload".to_vec();
        let h1 = compute_hash(None, &content);
        let h2 = compute_hash(Some(&h1), &content);
        assert_ne!(h1, h2, "different prev_hash must change the hash");
        assert_eq!(h1.len(), HASH_LEN);
    }

    #[test]
    fn canonical_bytes_are_deterministic() {
        let created = sample_created();
        let vals = (
            Some(Uuid::nil()),
            Uuid::nil(),
            "create",
            "transaction",
            Some(Uuid::nil()),
            Some(&json!({"a": 1})),
            Some(&json!({"a": 2})),
            created,
        );
        let a = canonical_bytes(
            vals.0, vals.1, vals.2, vals.3, vals.4, vals.5, vals.6, vals.7,
        );
        let b = canonical_bytes(
            vals.0, vals.1, vals.2, vals.3, vals.4, vals.5, vals.6, vals.7,
        );
        assert_eq!(a, b, "same inputs must serialize identically");
    }

    #[test]
    fn null_vs_present_distinguished() {
        let created = sample_created();
        let with_entity = canonical_bytes(
            None,
            Uuid::nil(),
            "x",
            "y",
            Some(Uuid::nil()),
            None,
            None,
            created,
        );
        let without = canonical_bytes(None, Uuid::nil(), "x", "y", None, None, None, created);
        assert_ne!(
            with_entity, without,
            "present vs absent entity_id must differ"
        );
    }
}
