//! Push notification abstractions.
//!
//! The `Notifier` trait is the single interface for sending push
//! notifications. Implementations: `ApnsNotifier`, `FcmNotifier`,
//! `NoopNotifier`.

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

pub mod apns;
pub mod fcm;
pub mod noop;
pub mod preferences;

// ─── Types ────────────────────────────────────────────────────────────────

/// A device token row from the DB.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeviceToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub platform: String,
}

#[derive(Debug, Error)]
pub enum NotifierError {
    #[error("APNs error: {0}")]
    Apns(String),
    #[error("FCM error: {0}")]
    Fcm(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("configuration error: {0}")]
    Config(String),
}

// ─── Trait ────────────────────────────────────────────────────────────────

#[async_trait]
pub trait Notifier: Send + Sync {
    /// Send a push notification to a device.
    async fn push(
        &self,
        device: &DeviceToken,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), NotifierError>;
}

/// Build a notifier from the `PUSH_PROVIDER` env var.
///
/// `"apns"` → APNs, `"fcm"` → FCM, anything else → noop.
pub fn from_env() -> Box<dyn Notifier> {
    match std::env::var("PUSH_PROVIDER").as_deref() {
        Ok("apns") => Box::new(apns::ApnsNotifier::from_env()),
        Ok("fcm") => Box::new(fcm::FcmNotifier::from_env()),
        _ => Box::new(noop::NoopNotifier),
    }
}

// ─── In-app + generic HTTP channels (`automation-platform`) ─────────────

/// Record an in-app notification for a user. Best-effort: failures are
/// logged, never propagated (callers sit inside business paths).
pub async fn record_in_app(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    event: &str,
    message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO in_app_notifications (user_id, event, message) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(event)
        .bind(message)
        .execute(pool)
        .await?;
    Ok(())
}

/// Deliver one notification to the user's generic HTTP target
/// (ntfy/Gotify-compatible JSON body), if configured. Returns
/// `Ok(false)` when no target exists.
pub async fn send_http_target(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    title: &str,
    body: &str,
) -> Result<bool, NotifierError> {
    let target: Option<(String,)> =
        sqlx::query_as("SELECT target_url FROM notification_http_targets WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| NotifierError::Http(e.to_string()))?;
    let Some((url,)) = target else {
        return Ok(false);
    };
    if !url.starts_with("https://") {
        return Err(NotifierError::Config("HTTP target must be https".into()));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| NotifierError::Http(e.to_string()))?;
    client
        .post(&url)
        .json(&serde_json::json!({ "topic": "openaccounting", "title": title, "message": body }))
        .send()
        .await
        .map_err(|e| NotifierError::Http(e.to_string()))?
        .error_for_status()
        .map_err(|e| NotifierError::Http(e.to_string()))?;
    Ok(true)
}
