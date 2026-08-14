//! Plaid bank feed provider adapter.
//!
//! Uses the Plaid Link flow: the user goes through the client-side
//! Plaid Link JS widget, gets a `public_token`, and POSTs it to
//! `/ledgers/{id}/bank-feeds/plaid/link`. We exchange it for an
//! `access_token` via `/item/public_token/exchange` and store it
//! encrypted.
//!
//! Subsequent syncs use `/transactions/sync` with the stored cursor.

use async_trait::async_trait;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

/// Plaid provider. Credentials come from env vars at call time.
#[derive(Default)]
pub struct PlaidProvider;

impl PlaidProvider {
    fn base_url() -> String {
        match std::env::var("PLAID_ENV").as_deref() {
            Ok("sandbox") => "https://sandbox.plaid.com".into(),
            Ok("development") => "https://development.plaid.com".into(),
            _ => "https://production.plaid.com".into(),
        }
    }

    fn client_id() -> Result<String, BankFeedError> {
        std::env::var("PLAID_CLIENT_ID")
            .map_err(|_| BankFeedError::MissingCredential("PLAID_CLIENT_ID".into()))
    }

    fn secret() -> Result<String, BankFeedError> {
        std::env::var("PLAID_SECRET")
            .map_err(|_| BankFeedError::MissingCredential("PLAID_SECRET".into()))
    }
}

#[async_trait]
impl Provider for PlaidProvider {
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError> {
        let client_id = Self::client_id()?;
        let secret = Self::secret()?;
        let body = serde_json::json!({
            "client_id": client_id,
            "secret": secret,
            "public_token": public_token,
        });
        let resp = reqwest::Client::new()
            .post(format!("{}/item/public_token/exchange", Self::base_url()))
            .json(&body)
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;
        json["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BankFeedError::InvalidData("no access_token in response".into()))
    }

    async fn fetch_transactions(
        &self,
        access_token: &str,
        cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        let client_id = Self::client_id()?;
        let secret = Self::secret()?;
        let body = serde_json::json!({
            "client_id": client_id,
            "secret": secret,
            "access_token": access_token,
            "cursor": cursor,
        });
        let resp = reqwest::Client::new()
            .post(format!("{}/transactions/sync", Self::base_url()))
            .json(&body)
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let next_cursor = json["next_cursor"].as_str().map(|s| s.to_string());
        let added = json["added"].as_array().cloned().unwrap_or_default();

        let txns = added
            .iter()
            .filter_map(|t| {
                let id = t["transaction_id"].as_str()?.to_string();
                let date_str = t["date"].as_str()?;
                let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
                let amount_raw = t["amount"].as_f64()?;
                let amount = rust_decimal::Decimal::try_from(amount_raw).ok()?;
                let description = t["name"].as_str().unwrap_or("").to_string();
                let payee = t["merchant_name"].as_str().map(|s| s.to_string());
                Some(RemoteTransaction {
                    provider_txn_id: id,
                    date,
                    amount,
                    description,
                    payee,
                })
            })
            .collect();

        Ok((txns, next_cursor))
    }
}
