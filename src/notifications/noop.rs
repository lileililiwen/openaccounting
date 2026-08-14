//! No-op notifier — the default when `PUSH_PROVIDER` is not set.
//!
//! All push calls succeed silently so the rest of the application
//! works without any external notification credentials.

use async_trait::async_trait;

use super::{DeviceToken, Notifier, NotifierError};

pub struct NoopNotifier;

#[async_trait]
impl Notifier for NoopNotifier {
    async fn push(
        &self,
        _device: &DeviceToken,
        _title: &str,
        _body: &str,
        _data: serde_json::Value,
    ) -> Result<(), NotifierError> {
        Ok(())
    }
}
