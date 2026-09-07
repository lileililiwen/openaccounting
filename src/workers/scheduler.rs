//! Scheduler worker (`automation-platform`).
//!
//! A single loop that (a) enqueues daily housekeeping jobs
//! (`template_scan`, `invoice_reminder_scan`, weekly digests on
//! Mondays, nightly purge) and (b) claims and executes due jobs from
//! the `jobs` table. Safe to run in multiple instances — claiming uses
//! `FOR UPDATE SKIP LOCKED` and every handler is idempotent.

use std::time::Duration;

use chrono::Datelike;
use sqlx::PgPool;
use tracing::{info, warn};

/// How often the loop wakes up to claim work.
pub const TICK_SECONDS: u64 = 30;

pub fn run_scheduler_loop(pool: PgPool) {
    tokio::spawn(async move {
        info!("scheduler worker started (tick = {TICK_SECONDS}s)");
        let mut last_housekeeping_day = Option::<chrono::NaiveDate>::None;
        let tick = Duration::from_secs(TICK_SECONDS);
        loop {
            if let Err(e) = housekeeping(&pool, &mut last_housekeeping_day).await {
                warn!("scheduler housekeeping failed: {e}");
            }
            match crate::jobs::run_due(&pool, 50).await {
                Ok(0) => {}
                Ok(n) => info!("scheduler executed {n} job(s)"),
                Err(e) => warn!("scheduler claim failed: {e}"),
            }
            tokio::time::sleep(tick).await;
        }
    });
}

/// Enqueue the periodic jobs once per day (and once per ISO week for
/// digests). Idempotent: guarded by an in-memory day marker plus the
/// handlers' own idempotency.
async fn housekeeping(
    pool: &PgPool,
    last_day: &mut Option<chrono::NaiveDate>,
) -> Result<(), sqlx::Error> {
    let today = chrono::Utc::now().date_naive();
    if *last_day == Some(today) {
        return Ok(());
    }

    crate::jobs::enqueue(pool, "template_scan", serde_json::json!({}), None, None).await?;
    crate::jobs::enqueue(
        pool,
        "invoice_reminder_scan",
        serde_json::json!({}),
        None,
        None,
    )
    .await?;
    crate::jobs::enqueue(
        pool,
        "recurring_invoice_scan",
        serde_json::json!({}),
        None,
        None,
    )
    .await?;

    // Weekly digest on Mondays.
    if today.weekday() == chrono::Weekday::Mon {
        let users: Vec<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM users WHERE is_active = TRUE")
                .fetch_all(pool)
                .await?;
        for (user_id,) in users {
            crate::jobs::enqueue(
                pool,
                "weekly_digest",
                serde_json::json!({ "user_id": user_id }),
                None,
                None,
            )
            .await?;
        }
    }

    // Nightly purge of finished jobs older than 30 days.
    let _ = crate::jobs::purge_old(pool).await?;

    *last_day = Some(today);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tick_is_thirty_seconds() {
        assert_eq!(super::TICK_SECONDS, 30);
    }
}
