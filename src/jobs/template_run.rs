//! Recurring-template job (`automation-platform`).
//!
//! The daily `template_scan` job posts every due occurrence exactly
//! once: the unique index `uq_txn_template_due (template_id, txn_date)`
//! makes re-runs a no-op, so the scheduler and the manual
//! `/templates/process_due` route can race safely.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::JobError;

/// Post every active template whose `next_date` is due, then advance
/// it. Idempotent per `(template_id, due_date)`.
pub async fn scan_and_run(pool: &PgPool) -> Result<(), JobError> {
    let today = chrono::Utc::now().date_naive();
    let due: Vec<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        Option<String>,
        String,
        NaiveDate,
    )> = sqlx::query_as(
        r#"SELECT id, ledger_id, description, payee, reference, frequency, next_date
               FROM transaction_templates
               WHERE is_active = TRUE AND next_date <= $1"#,
    )
    .bind(today)
    .fetch_all(pool)
    .await?;

    for (template_id, ledger_id, description, payee, reference, frequency, next_date) in due {
        // Catch up every missed occurrence, not just the first.
        let mut cursor = next_date;
        let mut guard = 0;
        while cursor <= today && guard < 120 {
            guard += 1;
            match run_occurrence(
                pool,
                template_id,
                ledger_id,
                &description,
                &payee,
                &reference,
                cursor,
            )
            .await
            {
                Ok(true) => {}
                Ok(false) => break, // duplicate → already posted; stop catching up
                Err(e) => return Err(e),
            }
            cursor = advance_frequency(cursor, &frequency);
        }
        if guard >= 120 {
            tracing::warn!(%template_id, "template_scan: 120+ occurrences due; stopping catch-up");
        }
    }
    Ok(())
}

/// Post one occurrence. Returns `Ok(false)` when the occurrence was
/// already posted (unique index hit).
#[allow(clippy::too_many_arguments)]
pub async fn run_occurrence(
    pool: &PgPool,
    template_id: Uuid,
    ledger_id: Uuid,
    description: &str,
    payee: &Option<String>,
    reference: &Option<String>,
    due_date: NaiveDate,
) -> Result<bool, JobError> {
    let postings: Vec<(Uuid, String, Decimal, Option<String>)> = sqlx::query_as(
        "SELECT account_id, direction, amount, memo FROM template_postings WHERE template_id = $1",
    )
    .bind(template_id)
    .fetch_all(pool)
    .await?;
    if postings.len() < 2 {
        return Err(JobError::Failed(format!(
            "template {template_id} has fewer than two postings"
        )));
    }

    // System actor for auto-generated entries: the ledger owner.
    let (actor,): (Uuid,) = sqlx::query_as("SELECT owner_id FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(pool)
        .await
        .map_err(JobError::Db)?;

    let mut tx = pool.begin().await.map_err(JobError::Db)?;

    // DO NOTHING on conflict yields no row — that IS the duplicate case.
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, kind,
                                     currency, created_by, template_id)
           VALUES ($1, $2, $3, $4, $5, 'recurring',
                   (SELECT base_currency FROM ledgers WHERE id = $1),
                   $6, $7)
           ON CONFLICT (template_id, txn_date) WHERE template_id IS NOT NULL DO NOTHING
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(due_date)
    .bind(description)
    .bind(payee)
    .bind(reference)
    .bind(actor)
    .bind(template_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(JobError::Db)?;
    let Some((txn_id,)) = inserted else {
        tx.rollback().await.map_err(JobError::Db)?;
        return Ok(false);
    };

    for (account_id, direction, amount, memo) in &postings {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, direction, amount, memo)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(txn_id)
        .bind(account_id)
        .bind(direction)
        .bind(amount)
        .bind(memo)
        .execute(&mut *tx)
        .await
        .map_err(JobError::Db)?;
    }

    sqlx::query(
        "UPDATE transaction_templates SET next_date = $1, updated_at = now() WHERE id = $2",
    )
    .bind(advance_frequency(
        due_date,
        frequency_of(pool, template_id).await?.as_str(),
    ))
    .bind(template_id)
    .execute(&mut *tx)
    .await
    .map_err(JobError::Db)?;

    tx.commit().await.map_err(JobError::Db)?;

    let _ = crate::audit::log(
        pool,
        Some(ledger_id),
        actor,
        "auto_generate",
        "transaction",
        Some(txn_id),
        None,
        Some(serde_json::json!({
            "template_id": template_id,
            "due_date": due_date.to_string(),
        })),
    )
    .await;

    // Event: recurring postings are ordinary postings to subscribers.
    crate::jobs::events::emit(
        pool,
        ledger_id,
        "transaction.posted",
        serde_json::json!({ "transaction_id": txn_id, "source": "recurring" }),
    )
    .await;

    Ok(true)
}

async fn frequency_of(pool: &PgPool, template_id: Uuid) -> Result<String, JobError> {
    let (f,): (String,) =
        sqlx::query_as("SELECT frequency FROM transaction_templates WHERE id = $1")
            .bind(template_id)
            .fetch_one(pool)
            .await
            .map_err(JobError::Db)?;
    Ok(f)
}

/// Advance a due date by one frequency step (shared with the handler).
pub fn advance_frequency(date: NaiveDate, frequency: &str) -> NaiveDate {
    match frequency {
        "weekly" => date + chrono::Duration::weeks(1),
        "biweekly" => date + chrono::Duration::weeks(2),
        "monthly" => add_months(date, 1),
        "quarterly" => add_months(date, 3),
        "yearly" => add_months(date, 12),
        _ => date + chrono::Duration::weeks(1),
    }
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + (date.month0() as i32) + months;
    let year = total.div_euclid(12);
    let month0 = total.rem_euclid(12);
    let day = date.day().min(days_in_month(year, month0 as u32 + 1));
    NaiveDate::from_ymd_opt(year, month0 as u32 + 1, day).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_frequency_steps() {
        assert_eq!(advance_frequency(D(2026, 1, 5), "weekly"), D(2026, 1, 12));
        assert_eq!(advance_frequency(D(2026, 1, 5), "biweekly"), D(2026, 1, 19));
        assert_eq!(advance_frequency(D(2026, 1, 31), "monthly"), D(2026, 2, 28));
        assert_eq!(advance_frequency(D(2026, 1, 5), "quarterly"), D(2026, 4, 5));
        assert_eq!(advance_frequency(D(2024, 2, 29), "yearly"), D(2025, 2, 28));
    }

    fn D(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
}
