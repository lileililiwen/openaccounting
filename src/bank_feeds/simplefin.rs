//! SimpleFIN provider adapter.
//!
//! The user visits <https://beta.simplefin.org>, generates a token,
//! and pastes it into the link form. We POST to the setup URL to get
//! a permanent `access_url`, then GET `/accounts` with HTTP Basic auth.

use async_trait::async_trait;
use base64::Engine as _;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

#[derive(Default)]
pub struct SimplefinProvider;

#[async_trait]
impl Provider for SimplefinProvider {
    async fn exchange_token(&self, setup_token_b64: &str) -> Result<String, BankFeedError> {
        // Decode the base64 setup token to get the claim URL.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(setup_token_b64.trim())
            .map_err(|_| BankFeedError::InvalidData("invalid SimpleFIN setup token".into()))?;
        let claim_url = String::from_utf8(decoded)
            .map_err(|_| BankFeedError::InvalidData("setup token is not valid UTF-8".into()))?;

        // POST to the claim URL to get the permanent access URL.
        let resp = reqwest::Client::new()
            .post(claim_url.trim())
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let access_url = resp
            .text()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;
        Ok(access_url.trim().to_string())
    }

    async fn fetch_transactions(
        &self,
        access_url: &str,
        _cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        // The access_url is the base URL; append /accounts.
        let accounts_url = format!("{}/accounts", access_url.trim_end_matches('/'));

        let resp: serde_json::Value = reqwest::Client::new()
            .get(&accounts_url)
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let accounts = resp["accounts"].as_array().cloned().unwrap_or_default();
        let mut txns = Vec::new();

        for acct in &accounts {
            for t in acct["transactions"].as_array().unwrap_or(&vec![]) {
                let id = match t["id"].as_str() {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let timestamp = match t["posted"].as_i64() {
                    Some(ts) => ts,
                    None => continue,
                };
                let date = chrono::DateTime::from_timestamp(timestamp, 0)
                    .map(|dt| dt.date_naive())
                    .unwrap_or_default();
                let amount_raw = match t["amount"].as_f64() {
                    Some(a) => a,
                    None => continue,
                };
                let amount = match rust_decimal::Decimal::try_from(amount_raw) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let description = t["description"].as_str().unwrap_or("").to_string();
                txns.push(RemoteTransaction {
                    provider_txn_id: id,
                    date,
                    amount,
                    description,
                    payee: None,
                });
            }
        }

        Ok((txns, None))
    }
}
