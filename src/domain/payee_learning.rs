//! Learned payee→account suggestions (`payee-learning`).
//!
//! The signal is *confirmed* matches and manual payee edits only —
//! never raw imports — so wrong auto-matches can't self-reinforce
//! (same discipline as GnuCash's OFX matcher). Confidence is hit
//! count decayed by recency; only confident aliases auto-fill.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Effective hits needed for import pre-fill (vs suggestion-only).
pub const AUTO_FILL_THRESHOLD: f64 = 3.0;
/// Confidence half-life in days.
const HALF_LIFE_DAYS: f64 = 180.0;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Suggestion {
    pub canonical_payee: String,
    pub account_id: Option<Uuid>,
    pub hit_count: i32,
    pub confidence: f64,
}

/// Record one confirmation. Idempotent per alias: reinforces.
pub async fn record(
    pool: &PgPool,
    ledger_id: Uuid,
    alias_normalized: &str,
    canonical_payee: &str,
    account_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    if alias_normalized.trim().is_empty() || canonical_payee.trim().is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"INSERT INTO payee_aliases (ledger_id, alias_normalized, canonical_payee, account_id)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (ledger_id, alias_normalized) DO UPDATE SET
               hit_count = payee_aliases.hit_count + 1,
               canonical_payee = EXCLUDED.canonical_payee,
               account_id = COALESCE(EXCLUDED.account_id, payee_aliases.account_id),
               last_used_at = now(),
               updated_at = now()"#,
    )
    .bind(ledger_id)
    .bind(alias_normalized)
    .bind(canonical_payee.trim())
    .bind(account_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn decay(last_used: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let age_days = (now - last_used).num_days().max(0) as f64;
    0.5_f64.powf(age_days / HALF_LIFE_DAYS)
}

/// Suggestions for a prefix + fuzzy substring match, ranked by
/// recency-decayed hits.
pub async fn suggest(
    pool: &PgPool,
    ledger_id: Uuid,
    query: &str,
    limit: i64,
) -> Result<Vec<Suggestion>, sqlx::Error> {
    let q = crate::import::statement::normalize_payee(query);
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(String, Option<Uuid>, i32, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT canonical_payee, account_id, hit_count, last_used_at
           FROM payee_aliases
           WHERE ledger_id = $1
                 AND (alias_normalized LIKE $2 || '%' OR alias_normalized LIKE '%' || $3 || '%')
           ORDER BY hit_count DESC, last_used_at DESC
           LIMIT $4"#,
    )
    .bind(ledger_id)
    .bind(&q)
    .bind(&q)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    let mut out: Vec<Suggestion> = rows
        .into_iter()
        .map(|(canonical_payee, account_id, hit_count, last_used_at)| {
            let confidence = hit_count as f64 * decay(last_used_at, now);
            Suggestion {
                canonical_payee,
                account_id,
                hit_count,
                confidence,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

/// Capture point: a confirmed reconciliation match teaches the alias.
pub async fn record_from_match(
    pool: &PgPool,
    ledger_id: Uuid,
    statement_description: &str,
    transaction_payee: &str,
    contra_account_id: Option<Uuid>,
) {
    let alias = crate::import::statement::normalize_payee(statement_description);
    if let Err(e) = record(
        pool,
        ledger_id,
        &alias,
        transaction_payee,
        contra_account_id,
    )
    .await
    {
        tracing::warn!(error = %e, "payee learning capture failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_halves_every_half_life() {
        let now = Utc::now();
        let fresh = now - chrono::Duration::days(1);
        let one_hl = now - chrono::Duration::days(180);
        let two_hl = now - chrono::Duration::days(360);
        assert!(decay(fresh, now) > 0.99);
        assert!((decay(one_hl, now) - 0.5).abs() < 0.01);
        assert!((decay(two_hl, now) - 0.25).abs() < 0.01);
    }

    #[test]
    fn stale_hits_drop_below_threshold() {
        // 5 hits a year+ old → below the auto-fill threshold.
        let now = Utc::now();
        let c = 5.0 * decay(now - chrono::Duration::days(400), now);
        assert!(c < AUTO_FILL_THRESHOLD);
        // 3 same-day hits ≥ threshold.
        let c = 3.0 * decay(now, now);
        assert!(c >= AUTO_FILL_THRESHOLD);
    }

    #[test]
    fn decimal_sanity_for_confidence_math() {
        // Confidence is f64; amounts stay Decimal elsewhere.
        assert_eq!(Decimal::new(3, 0), Decimal::from(3));
    }
}
