//! Outgoing webhook subscription management (`automation-platform`).

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use rand::RngCore;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    jobs::events::EVENT_TYPES,
    templates::webhooks::{WebhookList, WebhookRow},
    AppState,
};

/// Parsed `application/x-www-form-urlencoded` body. Checkbox groups
/// may legitimately carry one or many `events` values, which
/// `serde_urlencoded` cannot express — so we parse the body directly.
struct SubscriptionBody {
    target_url: String,
    events: Vec<String>,
}

impl SubscriptionBody {
    fn parse(body: &str) -> Self {
        let pairs: Vec<(String, String)> = form_urlencoded::parse(body.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        SubscriptionBody {
            target_url: pairs
                .iter()
                .find(|(k, _)| k == "target_url")
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            events: pairs
                .iter()
                .filter(|(k, _)| k == "events")
                .map(|(_, v)| v.clone())
                .collect(),
        }
    }
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rows: Vec<WebhookRow> = sqlx::query_as(
        r#"SELECT id, target_url, events, is_enabled,
                  (SELECT COUNT(*) FROM webhook_deliveries d
                    WHERE d.subscription_id = w.id AND d.ok = FALSE) AS failed_count
           FROM webhook_subscriptions w
           WHERE ledger_id = $1
           ORDER BY created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(WebhookList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "transactions".to_string(),
        subscriptions: rows,
        event_types: EVENT_TYPES.iter().map(|s| s.to_string()).collect(),
        new_secret: String::new(),
        error: String::new(),
    }))
}

/// Create a subscription. The signing secret is generated server-side
/// and shown exactly once (rendered back on the redirect page via
/// `new_secret`).
pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    body: String,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let form = SubscriptionBody::parse(&body);

    let url = form.target_url.trim();
    if !url.starts_with("https://") {
        return Err(AppError::Validation("Target URL must be https://".into()));
    }
    let mut events: Vec<String> = form
        .events
        .iter()
        .map(|e| e.trim().to_string())
        .filter(|e| EVENT_TYPES.contains(&e.as_str()))
        .collect();
    events.sort();
    events.dedup();
    if events.is_empty() {
        return Err(AppError::Validation(
            "Select at least one event type".into(),
        ));
    }

    let mut secret_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret_bytes);
    let secret = format!("whsec_{}", hex::encode(secret_bytes));

    sqlx::query(
        "INSERT INTO webhook_subscriptions (ledger_id, target_url, secret, events, created_by)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ledger_id)
    .bind(url)
    .bind(&secret)
    .bind(&events)
    .bind(user.id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::Db(e))?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "webhook_subscription",
        None,
        None,
        Some(serde_json::json!({ "target_url": url, "events": events })),
    )
    .await;

    // Show the secret once: render the list page with it populated.
    let rows: Vec<WebhookRow> = sqlx::query_as(
        r#"SELECT id, target_url, events, is_enabled,
                  0::BIGINT AS failed_count
           FROM webhook_subscriptions WHERE ledger_id = $1 ORDER BY created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(WebhookList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "transactions".to_string(),
        subscriptions: rows,
        event_types: EVENT_TYPES.iter().map(|s| s.to_string()).collect(),
        new_secret: secret,
        error: String::new(),
    }))
}

pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, sub_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    sqlx::query(
        "UPDATE webhook_subscriptions SET is_enabled = NOT is_enabled
         WHERE id = $1 AND ledger_id = $2",
    )
    .bind(sub_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/webhooks")).into_response())
}

/// Rotate the signing secret. The old secret stops verifying
/// immediately; the new one is shown once.
pub async fn rotate_secret(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, sub_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut secret_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret_bytes);
    let secret = format!("whsec_{}", hex::encode(secret_bytes));
    sqlx::query("UPDATE webhook_subscriptions SET secret = $1 WHERE id = $2 AND ledger_id = $3")
        .bind(&secret)
        .bind(sub_id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await?;

    let rows: Vec<WebhookRow> = sqlx::query_as(
        r#"SELECT id, target_url, events, is_enabled,
                  (SELECT COUNT(*) FROM webhook_deliveries d
                    WHERE d.subscription_id = w.id AND d.ok = FALSE) AS failed_count
           FROM webhook_subscriptions w WHERE ledger_id = $1 ORDER BY created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(WebhookList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "transactions".to_string(),
        subscriptions: rows,
        event_types: EVENT_TYPES.iter().map(|s| s.to_string()).collect(),
        new_secret: secret,
        error: String::new(),
    }))
}

/// Replay the latest failed delivery of a subscription: appends a
/// fresh attempt row and enqueues its job.
pub async fn replay_latest(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, sub_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // Latest failed attempt across all events of this subscription.
    let last: Option<(uuid::Uuid, i32)> = sqlx::query_as(
        "SELECT d.id, d.attempt
         FROM webhook_deliveries d
         JOIN webhook_subscriptions s ON s.id = d.subscription_id
         WHERE s.id = $1 AND s.ledger_id = $2 AND d.ok = FALSE
         ORDER BY d.created_at DESC LIMIT 1",
    )
    .bind(sub_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((last_id, last_attempt)) = last else {
        return Err(AppError::NotFound);
    };

    let (next_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO webhook_deliveries
            (subscription_id, event_id, event_type, payload, attempt)
         SELECT subscription_id, event_id, event_type, payload, $2 + 1
         FROM webhook_deliveries WHERE id = $1
         RETURNING id",
    )
    .bind(last_id)
    .bind(last_attempt)
    .fetch_one(&state.pool)
    .await?;

    crate::jobs::enqueue(
        &state.pool,
        "webhook_delivery",
        serde_json::json!({ "delivery_id": next_id }),
        None,
        Some(user.id),
    )
    .await?;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/webhooks")).into_response())
}
