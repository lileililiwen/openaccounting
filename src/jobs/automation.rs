//! Automation rule engine (`openapi-sdk`).
//!
//! Rules map an allowlisted trigger (internal event types plus signed
//! incoming `external.*` events) plus simple conditions to allowlisted
//! actions. A matching event enqueues one `automation_action` job on
//! the existing scheduler queue (retry/backoff come for free); the
//! executor dispatches the action (webhook delivery, email, or
//! transaction categorization) and writes an audit row.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::JobError;

/// Inbound-only event types accepted by `POST /api/events/{ledger_id}`.
pub const INBOUND_EVENT_TYPES: [&str; 3] = [
    "external.sync",
    "external.document.received",
    "external.payment.received",
];

/// Every trigger a rule may use: internal events plus inbound types.
pub fn allowed_triggers() -> Vec<&'static str> {
    let mut t: Vec<&'static str> = super::events::EVENT_TYPES.to_vec();
    t.extend_from_slice(&INBOUND_EVENT_TYPES);
    t
}

/// Allowlisted action kinds.
pub const ACTIONS: [&str; 3] = ["webhook_post", "email_notify", "categorize_transaction"];

/// Per-ledger cap on rule-generated jobs per hour (abuse guard).
pub const LEDGER_ACTION_BUDGET_PER_HOUR: i64 = 120;

/// Evaluate one rule's conditions against event data. An empty
/// condition object matches everything. Unknown fields fail closed.
pub fn conditions_match(conditions: &Value, data: &Value) -> bool {
    let Some(map) = conditions.as_object() else {
        return false;
    };
    for (key, expected) in map {
        match key.as_str() {
            "description_contains" => {
                let Some(needle) = expected.as_str() else {
                    return false;
                };
                let hay = data
                    .get("description")
                    .or_else(|| data.get("data").and_then(|d| d.get("description")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !hay.to_lowercase().contains(&needle.to_lowercase()) {
                    return false;
                }
            }
            "amount_gte" => {
                let Some(min) = expected.as_f64() else {
                    return false;
                };
                match event_number(data, "amount") {
                    Some(v) if v >= min => {}
                    _ => return false,
                }
            }
            "amount_lte" => {
                let Some(max) = expected.as_f64() else {
                    return false;
                };
                match event_number(data, "amount") {
                    Some(v) if v <= max => {}
                    _ => return false,
                }
            }
            "days_past_due_gte" => {
                let Some(min) = expected.as_f64() else {
                    return false;
                };
                match event_number(data, "days_past_due") {
                    Some(v) if v >= min => {}
                    _ => return false,
                }
            }
            _ => return false,
        }
    }
    true
}

fn event_number(data: &Value, field: &str) -> Option<f64> {
    data.get(field)
        .or_else(|| data.get("data").and_then(|d| d.get(field)))
        .and_then(|v| v.as_f64())
}

/// Match enabled rules for `(ledger, trigger)` and enqueue one
/// `automation_action` job per match, subject to the per-ledger hourly
/// budget. Returns the number of enqueued jobs. Best-effort: errors
/// are logged, never propagated (called from business write paths).
pub async fn match_and_enqueue(
    pool: &PgPool,
    ledger_id: Uuid,
    event_type: &str,
    data: &Value,
) -> i64 {
    let rules: Vec<(Uuid, Value, String, Value)> = match sqlx::query_as(
        "SELECT id, conditions, action, action_config FROM automation_rules
         WHERE ledger_id = $1 AND trigger = $2 AND is_enabled = TRUE",
    )
    .bind(ledger_id)
    .bind(event_type)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(%ledger_id, %event_type, error = %e, "automation rule lookup failed");
            return 0;
        }
    };
    if rules.is_empty() {
        return 0;
    }
    let used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs
         WHERE kind = 'automation_action'
           AND payload->>'ledger_id' = $1
           AND created_at > now() - INTERVAL '1 hour'",
    )
    .bind(ledger_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if used >= LEDGER_ACTION_BUDGET_PER_HOUR {
        tracing::warn!(%ledger_id, "automation action budget exhausted; skipping fan-out");
        return 0;
    }
    let mut enqueued = 0i64;
    for (rule_id, conditions, action, action_config) in rules {
        if !conditions_match(&conditions, data) {
            continue;
        }
        let payload = serde_json::json!({
            "rule_id": rule_id,
            "ledger_id": ledger_id,
            "event_type": event_type,
            "action": action,
            "action_config": action_config,
            "data": data,
        });
        if super::enqueue(pool, "automation_action", payload, None, None)
            .await
            .is_ok()
        {
            enqueued += 1;
        }
    }
    enqueued
}

/// Execute one `automation_action` job.
pub async fn run(pool: &PgPool, payload: &Value) -> Result<(), JobError> {
    let rule_id: Uuid = super::payload_field(payload, "rule_id")?;
    let ledger_id: Uuid = super::payload_field(payload, "ledger_id")?;
    let event_type: String = super::payload_field(payload, "event_type")?;
    let action: String = super::payload_field(payload, "action")?;
    let action_config: Value = payload
        .get("action_config")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let data: Value = payload.get("data").cloned().unwrap_or(Value::Null);

    // Re-check the rule is still enabled before acting.
    let enabled: Option<bool> =
        sqlx::query_scalar("SELECT is_enabled FROM automation_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_optional(pool)
            .await?;
    if enabled != Some(true) {
        return Ok(());
    }

    match action.as_str() {
        "webhook_post" => {
            let subscription_id: Uuid = super::payload_field(&action_config, "subscription_id")?;
            let envelope = serde_json::json!({
                "id": Uuid::new_v4(),
                "type": event_type,
                "occurred_at": chrono::Utc::now().to_rfc3339(),
                "ledger_id": ledger_id,
                "rule_id": rule_id,
                "data": data,
            });
            let (delivery_id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO webhook_deliveries
                    (subscription_id, event_id, event_type, payload, attempt)
                 VALUES ($1, $2, $3, $4, 0) RETURNING id",
            )
            .bind(subscription_id)
            .bind(
                envelope["id"]
                    .as_str()
                    .unwrap_or_default()
                    .parse::<Uuid>()
                    .unwrap_or_else(|_| Uuid::new_v4()),
            )
            .bind(&event_type)
            .bind(&envelope)
            .fetch_one(pool)
            .await?;
            super::enqueue(
                pool,
                "webhook_delivery",
                serde_json::json!({ "delivery_id": delivery_id }),
                None,
                None,
            )
            .await?;
        }
        "email_notify" => {
            let to: String = super::payload_field(&action_config, "to")?;
            let subject = format!("[OpenAccounting] {event_type}");
            let text = format!(
                "Automation rule fired for ledger {ledger_id}.\nEvent: {event_type}\nData: {data}"
            );
            super::enqueue(
                pool,
                "email_send",
                serde_json::json!({ "to": to, "subject": subject, "text": text }),
                None,
                None,
            )
            .await?;
        }
        "categorize_transaction" => {
            let cost_center_name: String = super::payload_field(&action_config, "cost_center")?;
            let txn_id: Option<Uuid> = data
                .get("transaction_id")
                .or_else(|| data.get("data").and_then(|d| d.get("transaction_id")))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let Some(txn_id) = txn_id else {
                return Err(JobError::Payload(
                    "categorize_transaction needs data.transaction_id".into(),
                ));
            };
            let cc_id: Uuid = sqlx::query_scalar(
                "INSERT INTO cost_centers (ledger_id, name) VALUES ($1, $2)
                 ON CONFLICT (ledger_id, name) DO UPDATE SET name = EXCLUDED.name
                 RETURNING id",
            )
            .bind(ledger_id)
            .bind(&cost_center_name)
            .fetch_one(pool)
            .await?;
            let updated = sqlx::query(
                "UPDATE postings SET cost_center_id = $2
                 WHERE transaction_id = $1 AND cost_center_id IS NULL",
            )
            .bind(txn_id)
            .bind(cc_id)
            .execute(pool)
            .await?;
            tracing::info!(%txn_id, %cost_center_name, rows = updated.rows_affected(), "automation categorized transaction");
        }
        other => return Err(JobError::Failed(format!("unknown action '{other}'"))),
    }

    let actor: Option<Uuid> =
        sqlx::query_scalar("SELECT created_by FROM automation_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_optional(pool)
            .await?;
    if let Some(actor) = actor {
        let _ = crate::audit::log(
            pool,
            Some(ledger_id),
            actor,
            "automation_rule_run",
            "automation_rule",
            Some(rule_id),
            None,
            Some(serde_json::json!({
                "event_type": event_type,
                "action": action,
            })),
        )
        .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_conditions_match_everything() {
        assert!(conditions_match(
            &serde_json::json!({}),
            &serde_json::json!({})
        ));
    }

    #[test]
    fn amount_conditions_compare_numerically() {
        let data = serde_json::json!({ "amount": 500 });
        assert!(conditions_match(
            &serde_json::json!({"amount_gte": 100}),
            &data
        ));
        assert!(!conditions_match(
            &serde_json::json!({"amount_gte": 900}),
            &data
        ));
        assert!(conditions_match(
            &serde_json::json!({"amount_lte": 900}),
            &data
        ));
        assert!(!conditions_match(
            &serde_json::json!({"amount_lte": 100}),
            &data
        ));
    }

    #[test]
    fn description_conditions_are_case_insensitive_contains() {
        let data = serde_json::json!({ "description": "Office Supplies Co." });
        assert!(conditions_match(
            &serde_json::json!({"description_contains": "office"}),
            &data
        ));
        assert!(!conditions_match(
            &serde_json::json!({"description_contains": "travel"}),
            &data
        ));
    }

    #[test]
    fn unknown_condition_field_fails_closed() {
        assert!(!conditions_match(
            &serde_json::json!({"mystery": 1}),
            &serde_json::json!({})
        ));
    }

    #[test]
    fn triggers_are_allowlisted() {
        let t = allowed_triggers();
        assert!(t.contains(&"transaction.posted"));
        assert!(t.contains(&"invoice.overdue"));
        assert!(t.contains(&"external.sync"));
        assert!(!t.contains(&"anything.goes"));
    }
}
