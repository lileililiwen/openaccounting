//! DB-backed background job queue (`automation-platform`).
//!
//! Jobs are rows in `jobs`; the scheduler worker (`src/workers/
//! scheduler.rs`) claims due rows with `FOR UPDATE SKIP LOCKED`, so
//! two server instances never run the same job. Failed jobs retry on
//! exponential backoff (30 s, 2 m, 10 m, 1 h, 6 h) and end `dead`
//! after `max_attempts`.
//!
//! Handlers live in sibling modules and are dispatched by [`run_due`].

pub mod digest;
pub mod email_send;
pub mod events;
pub mod invoice_reminders;
pub mod recurring_invoices;
pub mod template_run;
pub mod webhook_delivery;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Backoff before attempt N+1 (0-indexed attempts already made).
pub const BACKOFF_SECONDS: [i64; 5] = [30, 120, 600, 3600, 21_600];

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("job payload invalid: {0}")]
    Payload(String),
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl From<JobError> for sqlx::Error {
    fn from(e: JobError) -> Self {
        match e {
            JobError::Db(e) => e,
            other => sqlx::Error::Configuration(other.to_string().into()),
        }
    }
}

/// Enqueue one job. Returns its id.
pub async fn enqueue(
    pool: &PgPool,
    kind: &str,
    payload: Value,
    run_at: Option<DateTime<Utc>>,
    created_by: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO jobs (kind, payload, run_at, created_by)
         VALUES ($1, $2, COALESCE($3, now()), $4)
         RETURNING id",
    )
    .bind(kind)
    .bind(payload)
    .bind(run_at)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// One claimed job ready to execute.
#[derive(Debug)]
pub struct ClaimedJob {
    pub id: Uuid,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
}

/// Claim up to `limit` due jobs. `FOR UPDATE SKIP LOCKED` guarantees a
/// single winner across concurrent instances; the transaction commits
/// immediately after marking `running`.
pub async fn claim_due(pool: &PgPool, limit: i64) -> Result<Vec<ClaimedJob>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let rows: Vec<(Uuid, String, Value, i32, i32)> = sqlx::query_as(
        "WITH due AS (
             SELECT id FROM jobs
             WHERE status = 'pending' AND run_at <= now()
             ORDER BY run_at
             LIMIT $1
             FOR UPDATE SKIP LOCKED
         )
         UPDATE jobs j SET status = 'running', updated_at = now()
         FROM due
         WHERE j.id = due.id
         RETURNING j.id, j.kind, j.payload, j.attempts, j.max_attempts",
    )
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, payload, attempts, max_attempts)| ClaimedJob {
            id,
            kind,
            payload,
            attempts,
            max_attempts,
        })
        .collect())
}

/// Execute one claimed job and finalize it (`done`, retry backoff, or
/// `dead`). Errors inside handlers are captured, never propagated.
pub async fn execute(pool: &PgPool, job: ClaimedJob) {
    let result: Result<(), JobError> = match job.kind.as_str() {
        "template_scan" => template_run::scan_and_run(pool).await,
        "invoice_reminder_scan" => invoice_reminders::scan(pool).await,
        "recurring_invoice_scan" => recurring_invoices::scan_and_run(pool).await,
        "webhook_delivery" => webhook_delivery::deliver(pool, &job.payload).await,
        "email_send" => email_send::send(pool, &job.payload).await,
        "weekly_digest" => digest::send_for_user(pool, &job.payload).await,
        other => Err(JobError::Failed(format!("unknown job kind '{other}'"))),
    };

    match result {
        Ok(()) => {
            let _ =
                sqlx::query("UPDATE jobs SET status = 'done', updated_at = now() WHERE id = $1")
                    .bind(job.id)
                    .execute(pool)
                    .await;
        }
        Err(e) => {
            let attempts = job.attempts + 1;
            if attempts >= job.max_attempts {
                let _ = sqlx::query(
                    "UPDATE jobs SET status = 'dead', attempts = $2, last_error = $3,
                            updated_at = now()
                     WHERE id = $1",
                )
                .bind(job.id)
                .bind(attempts)
                .bind(e.to_string())
                .execute(pool)
                .await;
            } else {
                let delay = BACKOFF_SECONDS
                    .get((attempts as usize).saturating_sub(1))
                    .copied()
                    .unwrap_or(*BACKOFF_SECONDS.last().unwrap_or(&21_600));
                let _ = sqlx::query(
                    "UPDATE jobs SET status = 'pending', attempts = $2, last_error = $3,
                            run_at = now() + make_interval(secs => $4), updated_at = now()
                     WHERE id = $1",
                )
                .bind(job.id)
                .bind(attempts)
                .bind(e.to_string())
                .bind(delay)
                .execute(pool)
                .await;
            }
        }
    }
}

/// Claim and run every currently-due job (bounded). Used by the
/// scheduler loop and by tests / the manual trigger route.
pub async fn run_due(pool: &PgPool, limit: i64) -> Result<usize, sqlx::Error> {
    let jobs = claim_due(pool, limit).await?;
    let n = jobs.len();
    for job in jobs {
        execute(pool, job).await;
    }
    Ok(n)
}

/// Purge finished jobs older than 30 days (called by the nightly prune).
pub async fn purge_old(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "DELETE FROM jobs
         WHERE status IN ('done', 'dead')
               AND updated_at < now() - INTERVAL '30 days'",
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Extract a typed field from a job payload.
pub(crate) fn payload_field<T: serde::de::DeserializeOwned>(
    payload: &Value,
    key: &str,
) -> Result<T, JobError> {
    serde_json::from_value(
        payload
            .get(key)
            .cloned()
            .ok_or_else(|| JobError::Payload(format!("missing '{key}'")))?,
    )
    .map_err(|e| JobError::Payload(format!("bad '{key}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_schedule_is_exponential() {
        assert_eq!(BACKOFF_SECONDS[0], 30);
        assert_eq!(BACKOFF_SECONDS[1], 120);
        assert_eq!(BACKOFF_SECONDS[2], 600);
        assert_eq!(BACKOFF_SECONDS[3], 3600);
        assert_eq!(BACKOFF_SECONDS[4], 21_600);
        // Beyond the table → clamp to the last value.
        let clamp = BACKOFF_SECONDS
            .get(9)
            .copied()
            .unwrap_or(*BACKOFF_SECONDS.last().unwrap());
        assert_eq!(clamp, 21_600);
    }

    #[test]
    fn payload_field_reports_missing_key() {
        let v = serde_json::json!({});
        assert!(payload_field::<String>(&v, "nope").is_err());
        let v = serde_json::json!({ "k": "v" });
        assert_eq!(payload_field::<String>(&v, "k").unwrap(), "v");
    }
}
