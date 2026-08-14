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
