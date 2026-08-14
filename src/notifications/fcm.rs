//! Firebase Cloud Messaging (FCM) notifier stub.

use async_trait::async_trait;

use super::{DeviceToken, Notifier, NotifierError};

/// FCM notifier. Reads `FCM_SERVICE_ACCOUNT_PATH` from the environment.
pub struct FcmNotifier {
    service_account_path: String,
}

impl FcmNotifier {
    pub fn from_env() -> Self {
        Self {
            service_account_path: std::env::var("FCM_SERVICE_ACCOUNT_PATH").unwrap_or_default(),
        }
    }
}

#[async_trait]
impl Notifier for FcmNotifier {
    async fn push(
        &self,
        device: &DeviceToken,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), NotifierError> {
        if self.service_account_path.is_empty() {
            return Err(NotifierError::Config(
                "FCM_SERVICE_ACCOUNT_PATH not set".into(),
            ));
        }
        // Build the FCM HTTP v1 request. Full implementation would
        // use a service-account JWT. For v1, log the intent.
        tracing::info!(
            "FCM push to device={} title={:?} body={:?} data={}",
            device.token,
            title,
            body,
            data,
        );
        Ok(())
    }
}
