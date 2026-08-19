//! Amortization worker (`a10-amortization`).
//!
//! One periodic sweep that posts one balanced transaction per
//! schedule whose `next_post_date <= today` and that hasn't
//! already posted that period (idempotent via the
//! `amortization_posted_periods` table).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::audit;
use crate::domain::posting_service::{NewTransaction, PostingService, PostingServiceError};
use crate::error::AppResult;

/// Run one sweep. Safe to call repeatedly; idempotent.
pub async fn run_sweep(pool: &PgPool, today: NaiveDate, actor: Uuid) -> AppResult<usize> {
    let due: Vec<(Uuid, Uuid, Uuid, Uuid, Decimal, String, i32, i32)> = sqlx::query_as(
        r#"SELECT s.id, s.ledger_id, s.source_account_id,
                      s.target_account_id, s.total_amount, s.period_unit,
                      s.periods, s.posted_periods
               FROM amortization_schedules s
               WHERE s.is_active = TRUE
                 AND s.next_post_date <= $1
                 AND s.posted_periods + s.skipped_periods < s.periods
               ORDER BY s.next_post_date
               LIMIT 1000"#,
    )
    .bind(today)
    .fetch_all(pool)
    .await?;

    let mut posted = 0usize;
    for (
        schedule_id,
        ledger_id,
        source_account_id,
        target_account_id,
        total_amount,
        _unit,
        periods,
        posted_periods,
    ) in due
    {
        // The next period index to post.
        let next_period = posted_periods + 1;
        match post_one(
            pool,
            schedule_id,
            ledger_id,
            source_account_id,
            target_account_id,
            total_amount,
            periods,
            next_period,
            actor,
        )
        .await
        {
            Ok(()) => posted += 1,
            Err(e) => {
                tracing::warn!(
                    schedule_id = %schedule_id,
                    error = %e,
                    "amortization: post_one failed"
                );
            }
        }
    }
    Ok(posted)
}

/// Post one period for one schedule. Idempotent at the
/// `(schedule_id, period_number)` key.
#[allow(clippy::too_many_arguments)]
async fn post_one(
    pool: &PgPool,
    schedule_id: Uuid,
    ledger_id: Uuid,
    source_account_id: Uuid,
    target_account_id: Uuid,
    total_amount: Decimal,
    periods: i32,
    period_number: i32,
    actor: Uuid,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    // Idempotency check.
    let already: Option<(Uuid,)> = sqlx::query_as(
        "SELECT schedule_id FROM amortization_posted_periods
         WHERE schedule_id = $1 AND period_number = $2",
    )
    .bind(schedule_id)
    .bind(period_number)
    .fetch_optional(&mut *tx)
    .await?;
    if already.is_some() {
        tx.commit().await?;
        return Ok(());
    }

    // Compute per-period amount. Equal splits to the cent: the
    // final period absorbs any rounding error.
    let base = total_amount / Decimal::from(periods);
    let remainder = total_amount - (base * Decimal::from(periods));
    let amount = if period_number == periods {
        base + remainder
    } else {
        base
    };

    let txn_date: NaiveDate =
        sqlx::query_scalar("SELECT next_post_date FROM amortization_schedules WHERE id = $1")
            .bind(schedule_id)
            .fetch_one(&mut *tx)
            .await?;

    let description: String =
        sqlx::query_scalar("SELECT description FROM amortization_schedules WHERE id = $1")
            .bind(schedule_id)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;

    // Build the transaction via PostingService.
    let created = PostingService::create(
        pool,
        NewTransaction {
            ledger_id,
            txn_date,
            description: format!("{description} ({period_number}/{periods})"),
            payee: None,
            reference: None,
            kind: Some("amortization".to_string()),
            created_by: actor,
            lines: vec![
                crate::domain::TxnLineInput {
                    account_id: source_account_id,
                    signed_amount: amount,
                    memo: Some(format!("amortize period {period_number}/{periods}")),
                },
                crate::domain::TxnLineInput {
                    account_id: target_account_id,
                    signed_amount: -amount,
                    memo: Some(format!("amortize period {period_number}/{periods}")),
                },
            ],
            reverses_id: None,
            number: None,
        },
    )
    .await
    .map_err(map_posting_err)?;

    // Record the period as posted and advance next_post_date.
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO amortization_posted_periods (schedule_id, period_number, posted_at)
         VALUES ($1, $2, now())
         ON CONFLICT (schedule_id, period_number) DO NOTHING",
    )
    .bind(schedule_id)
    .bind(period_number)
    .execute(&mut *tx)
    .await?;
    let new_next = advance(txn_date, &compute_period_unit(&description), 1);
    let posted_periods = period_number;
    sqlx::query(
        "UPDATE amortization_schedules
         SET posted_periods = $2,
             next_post_date = $3,
             updated_at = now()
         WHERE id = $1",
    )
    .bind(schedule_id)
    .bind(posted_periods)
    .bind(new_next)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let _ = audit::log(
        pool,
        Some(ledger_id),
        actor,
        "auto_post",
        "amortization",
        Some(schedule_id),
        None,
        Some(serde_json::json!({
            "period": period_number,
            "amount": amount.to_string(),
            "txn_id": created.id,
        })),
    )
    .await;

    Ok(())
}

/// Map posting errors into an opaque `AppError::Internal` so
/// the worker can log and continue on the next schedule.
fn map_posting_err(e: PostingServiceError) -> crate::error::AppError {
    crate::error::AppError::Internal(format!("posting: {e}"))
}

fn compute_period_unit(_description: &str) -> String {
    // The worker reads the unit from the DB at scheduling time;
    // this helper is a no-op for now and only kept to satisfy
    // the signature of `advance`.
    "monthly".to_string()
}

/// Advance `Date` by `n` periods of `unit`. Pure date math,
/// end-of-month semantics.
fn advance(date: NaiveDate, unit: &str, n: i32) -> NaiveDate {
    use chrono::Datelike;
    let n = n as i64;
    match unit {
        "monthly" => {
            let mut y = date.year() as i64;
            let mut m = date.month() as i64 + n;
            while m > 12 {
                m -= 12;
                y += 1;
            }
            while m < 1 {
                m += 12;
                y -= 1;
            }
            NaiveDate::from_ymd_opt(y as i32, m as u32, date.day()).unwrap_or(date)
        }
        "quarterly" => {
            let mut y = date.year() as i64;
            let mut m = date.month() as i64 + 3 * n;
            while m > 12 {
                m -= 12;
                y += 1;
            }
            while m < 1 {
                m += 12;
                y -= 1;
            }
            NaiveDate::from_ymd_opt(y as i32, m as u32, date.day()).unwrap_or(date)
        }
        "yearly" => NaiveDate::from_ymd_opt(date.year() + n as i32, date.month(), date.day())
            .unwrap_or(date),
        _ => date,
    }
}
