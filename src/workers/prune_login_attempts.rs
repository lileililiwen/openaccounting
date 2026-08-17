//! Daily worker that prunes `login_attempts` rows older than 90 days.
//!
//! Per the `s2-login-rate-limiting` spec, login attempts are kept
//! for 90 days for audit, then deleted. The worker is spawned once
//! at startup (see `crate::run`) and loops forever, sleeping 24
//! hours between runs.
//!
//! Exposed entry points:
//! * [`run_prune_loop`] — spawn-and-forget loop.
//! * [`run_once`] — run a single prune pass; used by tests and
//!   called from the loop. Returns the number of rows deleted.

use std::time::Duration;

use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::{info, warn};

/// How long login attempts are retained.
pub const RETENTION_DAYS: i64 = 90;
/// How often the prune loop runs.
pub const PRUNE_INTERVAL_HOURS: u64 = 24;

/// Spawn a tokio task that prunes login attempts forever, once per
/// 24 hours. The initial sleep gives the server time to finish
/// booting before the first (potentially heavy) `DELETE` runs.
pub fn run_prune_loop(pool: PgPool) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(PRUNE_INTERVAL_HOURS * 3600);
        info!(
            "login-attempts prune worker started (retention = {RETENTION_DAYS}d, interval = {PRUNE_INTERVAL_HOURS}h)"
        );
        loop {
            tokio::time::sleep(interval).await;
            match run_once(&pool).await {
                Ok(n) if n > 0 => {
                    info!("pruned {n} login_attempts rows older than {RETENTION_DAYS}d")
                }
                Ok(_) => {}
                Err(e) => warn!("login-attempts prune failed: {e}"),
            }
        }
    });
}

/// Run a single prune pass. Deletes rows whose `ts` is older than
/// [`RETENTION_DAYS`] days. Returns the count of rows deleted.
pub async fn run_once(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let cutoff: OffsetDateTime = OffsetDateTime::now_utc() - time::Duration::days(RETENTION_DAYS);
    let res = sqlx::query("DELETE FROM login_attempts WHERE ts < $1")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::rate_limit::record_attempt;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    async fn pool_or_skip() -> Option<PgPool> {
        let url = env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .ok()
    }

    #[tokio::test]
    async fn prune_deletes_old_attempts() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        sqlx::query("DELETE FROM login_attempts")
            .execute(&pool)
            .await
            .unwrap();

        // Three rows: one ancient (pruned), one on the boundary
        // (older than 90 days → pruned), one fresh (kept).
        let now = OffsetDateTime::now_utc();
        let ancient = now - time::Duration::days(365);
        let boundary = now - time::Duration::days(RETENTION_DAYS + 1);
        let fresh = now - time::Duration::days(7);

        record_attempt(&pool, "10.0.0.10", "ancient@example.com", false, ancient)
            .await
            .unwrap();
        record_attempt(&pool, "10.0.0.10", "boundary@example.com", false, boundary)
            .await
            .unwrap();
        record_attempt(&pool, "10.0.0.10", "fresh@example.com", false, fresh)
            .await
            .unwrap();

        let deleted = run_once(&pool).await.unwrap();
        assert!(
            deleted >= 2,
            "expected at least 2 rows deleted, got {deleted}"
        );

        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM login_attempts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 1, "only the fresh row should remain");

        // And the fresh row's email should still be there.
        let (e,): (String,) = sqlx::query_as("SELECT email FROM login_attempts WHERE email = $1")
            .bind("fresh@example.com")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(e, "fresh@example.com");

        // Cleanup so the row doesn't pollute the dev DB.
        sqlx::query("DELETE FROM login_attempts WHERE email = 'fresh@example.com'")
            .execute(&pool)
            .await
            .ok();
    }
}
