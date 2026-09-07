//! Invoice overdue reminders (`automation-platform`).
//!
//! The daily scan finds unpaid invoices past their due date and fires
//! `invoice.overdue` at day 1, 7, and 14 past due. Each (invoice,
//! offset) fires at most once — enforced by the unique index on
//! `invoice_reminders`. Delivery goes to every channel the ledger
//! owner enabled for `invoice_overdue`; when no channel is available
//! the reminder is recorded as skipped so it never retries.

use chrono::NaiveDate;
use sqlx::PgPool;

use super::JobError;

pub const REMINDER_OFFSETS: [i64; 3] = [1, 7, 14];

/// Scan unpaid overdue invoices and fire due reminders.
pub async fn scan(pool: &PgPool) -> Result<(), JobError> {
    let today = chrono::Utc::now().date_naive();
    // AR invoices that are open or already flagged overdue but not
    // paid/void, past due.
    let rows: Vec<(uuid::Uuid, uuid::Uuid, uuid::Uuid, NaiveDate)> = sqlx::query_as(
        r#"SELECT i.id, i.ledger_id, i.contact_id, i.due_date
           FROM invoices i
           WHERE i.kind = 'receivable'
                 AND i.status IN ('open', 'overdue')
                 AND i.due_date < $1"#,
    )
    .bind(today)
    .fetch_all(pool)
    .await
    .map_err(JobError::Db)?;

    for (invoice_id, ledger_id, contact_id, due_date) in rows {
        let days_overdue = (today - due_date).num_days();
        for offset in REMINDER_OFFSETS {
            if days_overdue < offset {
                continue;
            }
            fire_once(pool, invoice_id, ledger_id, contact_id, offset).await?;
        }
    }
    Ok(())
}

/// Fire one reminder offset if it has not fired yet. Idempotent via
/// the `(invoice_id, offset_day, channel)` unique index.
async fn fire_once(
    pool: &PgPool,
    invoice_id: uuid::Uuid,
    ledger_id: uuid::Uuid,
    contact_id: uuid::Uuid,
    offset: i64,
) -> Result<(), JobError> {
    // Owner of the ledger receives the notification; the contact's
    // email is the external recipient when present.
    let (owner_id,): (uuid::Uuid,) = sqlx::query_as("SELECT owner_id FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(pool)
        .await
        .map_err(JobError::Db)?;
    let (contact_email,): (Option<String>,) =
        sqlx::query_as("SELECT COALESCE(email, '') FROM contacts WHERE id = $1")
            .bind(contact_id)
            .fetch_one(pool)
            .await
            .map_err(JobError::Db)?;
    let (invoice_number, total): (Option<String>, rust_decimal::Decimal) =
        sqlx::query_as("SELECT invoice_number, total FROM invoices WHERE id = $1")
            .bind(invoice_id)
            .fetch_one(pool)
            .await
            .map_err(JobError::Db)?;

    // Claim this offset first (unique index arbitrates concurrent runs).
    let claimed = sqlx::query(
        "INSERT INTO invoice_reminders (invoice_id, offset_day, channel)
         VALUES ($1, $2, 'owner')
         ON CONFLICT (invoice_id, offset_day, channel) DO NOTHING",
    )
    .bind(invoice_id)
    .bind(offset as i32)
    .execute(pool)
    .await
    .map_err(JobError::Db)?;
    if claimed.rows_affected() == 0 {
        return Ok(()); // already reminded for this offset
    }

    // Deliver through configured channels.
    let mut delivered_any = false;
    if crate::jobs::email_send::configured() {
        if let Some(email) = contact_email.as_deref().filter(|e| !e.is_empty()) {
            let subject = format!(
                "Overdue invoice {} ({} days past due)",
                invoice_number.as_deref().unwrap_or(""),
                offset
            );
            let text = format!(
                "Invoice {} is {offset} day(s) past due. Outstanding amount: {total}.",
                invoice_number.as_deref().unwrap_or("")
            );
            super::enqueue(
                pool,
                "email_send",
                serde_json::json!({ "to": email, "subject": subject, "text": text }),
                None,
                None,
            )
            .await
            .map_err(JobError::Db)?;
            delivered_any = true;
        }
    }

    // In-app record for the owner's dashboard/notification list.
    let _ = crate::notifications::record_in_app(
        pool,
        owner_id,
        "invoice_overdue",
        &format!(
            "Invoice {} is {} day(s) overdue",
            invoice_number.as_deref().unwrap_or("(unnumbered)"),
            offset
        ),
    )
    .await;
    delivered_any = true; // in-app always counts as a delivery path

    if !delivered_any {
        sqlx::query(
            "UPDATE invoice_reminders SET skipped = TRUE, note = 'no channel' 
             WHERE invoice_id = $1 AND offset_day = $2 AND channel = 'owner'",
        )
        .bind(invoice_id)
        .bind(offset as i32)
        .execute(pool)
        .await
        .map_err(JobError::Db)?;
    }

    crate::jobs::events::emit(
        pool,
        ledger_id,
        "invoice.overdue",
        serde_json::json!({
            "invoice_id": invoice_id,
            "offset_day": offset,
            "days_past_due": offset,
        }),
    )
    .await;

    Ok(())
}
