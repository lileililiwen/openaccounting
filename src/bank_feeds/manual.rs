//! Manual (no-op) bank feed provider stub.
//!
//! This adapter requires no external credentials and always returns an
//! empty transaction list. It keeps zero-config self-hosted installs
//! working without third-party API keys.

use async_trait::async_trait;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

pub struct ManualProvider;

#[async_trait]
impl Provider for ManualProvider {
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError> {
        // No exchange needed; the token is passed through.
        Ok(public_token.to_string())
    }

    async fn fetch_transactions(
        &self,
        _access_token: &str,
        _cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        // Manual provider never fetches from an external API.
        Ok((vec![], None))
    }
}
