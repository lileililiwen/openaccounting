//! Outgoing event emission (`automation-platform`).
//!
//! `emit` fans an event out to every enabled subscription of the
//! ledger that subscribed to its type: one pending delivery row per
//! subscriber plus one `webhook_delivery` job. Best-effort — a failing
//! fan-out must never break the business write that triggered it.

use sqlx::PgPool;
use uuid::Uuid;

/// The event types receivers can subscribe to.
pub const EVENT_TYPES: [&str; 6] = [
    "transaction.posted",
    "transaction.voided",
    "invoice.created",
    "invoice.paid",
    "invoice.overdue",
    "budget.threshold_crossed",
];

/// Fan out one event. Errors are logged, never returned: callers run
/// inside business write paths.
pub async fn emit(pool: &PgPool, ledger_id: Uuid, event_type: &str, data: serde_json::Value) {
    if let Err(e) = emit_inner(pool, ledger_id, event_type, data).await {
        tracing::warn!(%ledger_id, %event_type, error = %e, "webhook fan-out failed");
    }
}

async fn emit_inner(
    pool: &PgPool,
    ledger_id: Uuid,
    event_type: &str,
    data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let subs: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM webhook_subscriptions
         WHERE ledger_id = $1 AND is_enabled = TRUE AND $2 = ANY(events)",
    )
    .bind(ledger_id)
    .bind(event_type)
    .fetch_all(pool)
    .await?;
    if subs.is_empty() {
        return Ok(());
    }

    let event_id = Uuid::new_v4();
    let occurred_at = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "id": event_id,
        "type": event_type,
        "occurred_at": occurred_at,
        "ledger_id": ledger_id,
        "data": data,
    });

    for (subscription_id,) in subs {
        // One delivery row per (subscription, event); attempt 0 is
        // created here, retries append further attempt rows.
        let (delivery_id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO webhook_deliveries
                (subscription_id, event_id, event_type, payload, attempt)
             VALUES ($1, $2, $3, $4, 0)
             ON CONFLICT (subscription_id, event_id, attempt) DO NOTHING
             RETURNING id",
        )
        .bind(subscription_id)
        .bind(event_id)
        .bind(event_type)
        .bind(&payload)
        .fetch_optional(pool)
        .await?
        .unwrap_or((Uuid::nil(),));

        if delivery_id.is_nil() {
            continue; // attempt 0 already existed (replay race)
        }

        super::enqueue(
            pool,
            "webhook_delivery",
            serde_json::json!({ "delivery_id": delivery_id }),
            None,
            None,
        )
        .await?;
    }
    Ok(())
}
