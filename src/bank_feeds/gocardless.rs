//! GoCardless Bank Account Data provider adapter.

use async_trait::async_trait;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

#[derive(Default)]
pub struct GoCardlessProvider;

#[async_trait]
impl Provider for GoCardlessProvider {
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError> {
        // GoCardless does not use a public_token exchange; the requisition ID
        // IS the persistent identifier. Return it unchanged.
        Ok(public_token.to_string())
    }

    async fn fetch_transactions(
        &self,
        access_token: &str,
        _cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        let secret_id = std::env::var("GOCARDLESS_SECRET_ID")
            .map_err(|_| BankFeedError::MissingCredential("GOCARDLESS_SECRET_ID".into()))?;
        let secret_key = std::env::var("GOCARDLESS_SECRET_KEY")
            .map_err(|_| BankFeedError::MissingCredential("GOCARDLESS_SECRET_KEY".into()))?;

        // Step 1: obtain a bearer token.
        let token_resp: serde_json::Value = reqwest::Client::new()
            .post("https://bankaccountdata.gocardless.com/api/v2/token/new/")
            .json(&serde_json::json!({
                "secret_id": secret_id,
                "secret_key": secret_key,
            }))
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let bearer = token_resp["access"]
            .as_str()
            .ok_or_else(|| BankFeedError::InvalidData("no access token in auth response".into()))?
            .to_string();

        // Step 2: fetch transactions for the account.
        let url = format!(
            "https://bankaccountdata.gocardless.com/api/v2/accounts/{access_token}/transactions/"
        );
        let resp: serde_json::Value = reqwest::Client::new()
            .get(&url)
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?
            .json()
            .await
            .map_err(|e| BankFeedError::Http(e.to_string()))?;

        let booked = resp["transactions"]["booked"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let txns = booked
            .iter()
            .filter_map(|t| {
                let id = t["transactionId"].as_str()?.to_string();
                let date_str = t["bookingDate"].as_str()?;
                let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
                let amount_str = t["transactionAmount"]["amount"].as_str()?;
                let amount: rust_decimal::Decimal = amount_str.parse().ok()?;
                let description = t["remittanceInformationUnstructured"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                Some(RemoteTransaction {
                    provider_txn_id: id,
                    date,
                    amount,
                    description,
                    payee: None,
                })
            })
            .collect();

        Ok((txns, None))
    }
}
