//! Salt Edge Connect provider adapter.

use async_trait::async_trait;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

#[derive(Default)]
pub struct SaltEdgeProvider;

#[async_trait]
impl Provider for SaltEdgeProvider {
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError> {
        // Salt Edge: the connection_id returned from the connect flow is
        // the persistent identifier. Return unchanged.
        Ok(public_token.to_string())
    }

    async fn fetch_transactions(
        &self,
        access_token: &str,
        cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        let app_id = std::env::var("SALT_EDGE_APP_ID")
            .map_err(|_| BankFeedError::MissingCredential("SALT_EDGE_APP_ID".into()))?;
        let secret = std::env::var("SALT_EDGE_SECRET")
            .map_err(|_| BankFeedError::MissingCredential("SALT_EDGE_SECRET".into()))?;

        let mut url =
            format!("https://www.saltedge.com/api/v5/transactions?connection_id={access_token}");
        if let Some(ref c) = cursor {
            url.push_str(&format!("&from_id={c}"));
        }

        let resp: serde_json::Value = reqwest::Client::new()
            .get(&url)
            .header("App-id", &app_id)
            .header("Secret", &secret)
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let data = resp["data"].as_array().cloned().unwrap_or_default();
        let next_cursor = data
            .last()
            .and_then(|t| t["id"].as_str())
            .map(|s| s.to_string());

        let txns = data
            .iter()
            .filter_map(|t| {
                let id = t["id"].as_str()?.to_string();
                let date_str = t["made_on"].as_str()?;
                let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
                let amount_raw = t["amount"].as_f64()?;
                let amount = rust_decimal::Decimal::try_from(amount_raw).ok()?;
                let description = t["description"].as_str().unwrap_or("").to_string();
                Some(RemoteTransaction {
                    provider_txn_id: id,
                    date,
                    amount,
                    description,
                    payee: None,
                })
            })
            .collect();

        Ok((txns, next_cursor))
    }
}
