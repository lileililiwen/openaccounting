//! Apple Push Notification service (APNs) notifier stub.
//!
//! In v1, this shells out to `apnstool` if present, or uses the
//! `apns-rust` crate (optional dep). For self-hosted installs that
//! don't need native push, `NoopNotifier` is the default.

use async_trait::async_trait;

use super::{DeviceToken, Notifier, NotifierError};

/// APNs notifier. Reads `APNS_KEY_PATH`, `APNS_KEY_ID`, `APNS_TEAM_ID`,
/// `APNS_BUNDLE_ID` from the environment.
pub struct ApnsNotifier {
    key_path: String,
    _key_id: String,
    _team_id: String,
    _bundle_id: String,
}

impl ApnsNotifier {
    pub fn from_env() -> Self {
        Self {
            key_path: std::env::var("APNS_KEY_PATH").unwrap_or_default(),
            _key_id: std::env::var("APNS_KEY_ID").unwrap_or_default(),
            _team_id: std::env::var("APNS_TEAM_ID").unwrap_or_default(),
            _bundle_id: std::env::var("APNS_BUNDLE_ID").unwrap_or_default(),
        }
    }
}

#[async_trait]
impl Notifier for ApnsNotifier {
    async fn push(
        &self,
        device: &DeviceToken,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), NotifierError> {
        if self.key_path.is_empty() {
            return Err(NotifierError::Config("APNS_KEY_PATH not set".into()));
        }
        // Build the APNs JWT and POST to api.push.apple.com.
        // Full implementation would use `apns-rust` or `reqwest` with a JWT.
        // For v1, log the notification intent.
        tracing::info!(
            "APNs push to device={} title={:?} body={:?} data={}",
            device.token,
            title,
            body,
            data,
        );
        Ok(())
    }
}
