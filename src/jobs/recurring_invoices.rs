//! Recurring invoice issuance job (`invoicing-completeness`).
//!
//! The daily scan issues one invoice per due recurring-invoice
//! template occurrence. Idempotent by the unique index
//! `uq_recurring_invoice_due (recurring_template_id, invoice_date)`.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::JobError;

/// Issue every due recurring invoice. Safe to run concurrently with
/// the manual trigger.
pub async fn scan_and_run(pool: &PgPool) -> Result<(), JobError> {
    let today = chrono::Utc::now().date_naive();
    let templates: Vec<(Uuid, Uuid, Uuid, String, String, NaiveDate)> = sqlx::query_as(
        "SELECT id, ledger_id, contact_id, kind, frequency, next_date
         FROM recurring_invoice_templates
         WHERE is_active = TRUE AND next_date <= $1",
    )
    .bind(today)
    .fetch_all(pool)
    .await
    .map_err(JobError::Db)?;

    for (template_id, ledger_id, contact_id, kind, frequency, next_date) in templates {
        let mut cursor = next_date;
        let mut guard = 0;
        while cursor <= today && guard < 120 {
            guard += 1;
            match issue_occurrence(pool, template_id, ledger_id, contact_id, &kind, cursor).await {
                Ok(true) => {}
                Ok(false) => break, // already issued → stop catch-up
                Err(e) => return Err(e),
            }
            cursor = crate::jobs::template_run::advance_frequency(cursor, &frequency);
        }
    }
    Ok(())
}

/// Issue one occurrence. Returns `Ok(false)` when it already exists.
#[allow(clippy::too_many_arguments)]
async fn issue_occurrence(
    pool: &PgPool,
    template_id: Uuid,
    ledger_id: Uuid,
    contact_id: Uuid,
    kind: &str,
    issue_date: NaiveDate,
) -> Result<bool, JobError> {
    let lines: Vec<(String, Decimal, Decimal)> = sqlx::query_as(
        "SELECT description, quantity, unit_price FROM recurring_invoice_lines
         WHERE template_id = $1 ORDER BY sort_order",
    )
    .bind(template_id)
    .fetch_all(pool)
    .await
    .map_err(JobError::Db)?;
    if lines.is_empty() {
        return Err(JobError::Failed(format!(
            "recurring invoice template {template_id} has no lines"
        )));
    }
    let total: Decimal = lines.iter().map(|(_, q, up)| (*q * *up).round_dp(2)).sum();

    let mut tx = pool.begin().await.map_err(JobError::Db)?;
    // DO NOTHING on conflict yields no row — that IS the duplicate case.
    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"INSERT INTO invoices (ledger_id, contact_id, kind, invoice_date, due_date, total,
                                 status, recurring_template_id)
           VALUES ($1, $2, $3, $4, $4, $5, 'open', $6)
           ON CONFLICT (recurring_template_id, invoice_date) WHERE recurring_template_id IS NOT NULL
           DO NOTHING RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(contact_id)
    .bind(kind)
    .bind(issue_date)
    .bind(total)
    .bind(template_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(JobError::Db)?;
    let Some((invoice_id,)) = existing else {
        tx.rollback().await.map_err(JobError::Db)?;
        return Ok(false);
    };

    for (i, (description, quantity, unit_price)) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invoice_id)
        .bind(description)
        .bind(quantity)
        .bind(unit_price)
        .bind((*quantity * *unit_price).round_dp(2))
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .map_err(JobError::Db)?;
    }
    tx.commit().await.map_err(JobError::Db)?;

    crate::jobs::events::emit(
        pool,
        ledger_id,
        "invoice.created",
        serde_json::json!({ "invoice_id": invoice_id, "kind": kind, "total": total }),
    )
    .await;
    Ok(true)
}

#[allow(dead_code)]
fn _assert_datelike_in_scope(d: impl Datelike) {}
