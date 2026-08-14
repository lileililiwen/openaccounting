//! HTTP handlers for push-notification device registration.
//!
//! Routes:
//!   POST /devices/register    — store an APNs/FCM token for the current user
//!   POST /devices/unregister  — remove a token
//!   POST /devices/push        — dispatch a push to all devices of a user
//!                               (internal; called by other handlers)

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_login::AuthSession;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    notifications::{from_env, DeviceToken},
    AppState,
};

// ─── Register ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub token: String,
    pub platform: String,
}

pub async fn register(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let valid_platforms = ["ios", "android", "web"];
    if !valid_platforms.contains(&body.platform.as_str()) {
        return Err(AppError::Validation(format!(
            "unknown platform: {}",
            body.platform
        )));
    }

    sqlx::query(
        r#"INSERT INTO device_tokens (user_id, token, platform)
           VALUES ($1, $2, $3)
           ON CONFLICT (token) DO UPDATE SET user_id = $1, platform = $3, updated_at = now()"#,
    )
    .bind(user.id)
    .bind(&body.token)
    .bind(&body.platform)
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({"ok": true}))).into_response())
}

// ─── Unregister ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UnregisterRequest {
    pub token: String,
}

pub async fn unregister(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Json(body): Json<UnregisterRequest>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    sqlx::query("DELETE FROM device_tokens WHERE token = $1 AND user_id = $2")
        .bind(&body.token)
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

// ─── Dispatcher ────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize)]
pub struct PushRequest {
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
}

/// Dispatch a push notification to all devices registered for `user_id`.
///
/// Called internally (e.g. from approval routing, OCR completion).
/// Always succeeds — failures are logged but do not surface to the caller.
pub async fn dispatch(state: &AppState, req: PushRequest) {
    let devices: Vec<DeviceToken> =
        match sqlx::query_as("SELECT id, user_id, token, platform FROM device_tokens WHERE user_id = $1")
            .bind(req.user_id)
            .fetch_all(&state.pool)
            .await
        {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("dispatch: failed to load devices: {e}");
                return;
            }
        };

    if devices.is_empty() {
        return;
    }

    let notifier = from_env();
    for device in &devices {
        if let Err(e) = notifier
            .push(device, &req.title, &req.body, req.data.clone())
            .await
        {
            tracing::warn!("dispatch: push failed for device {}: {e}", device.id);
        }
    }
}
