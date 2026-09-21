//! Enable Banking (EU open-banking aggregator) provider adapter.
//!
//! Second documented EU integration shape behind the [`Provider`](super::Provider)
//! trait (see the trait docs in `super` for the field-mapping table).
//! Live credentials cannot be tested in CI, so this adapter maps the
//! documented Enable Banking transaction JSON shape to
//! [`RemoteTransaction`](super::RemoteTransaction) without performing
//! live OAuth — `fetch_transactions` parses the `access_token` argument
//! as a JSON payload in tests and returns `MissingCredential` in
//! production-shaped calls. A follow-up change can swap the stub fetch
//! for a live client without touching the trait.

use async_trait::async_trait;

use super::{BankFeedError, Cursor, Provider, RemoteTransaction};

#[derive(Default)]
pub struct EnableBankingProvider;

/// Map one Enable Banking transaction object to the normalized shape.
///
/// Expected JSON per transaction:
/// `{ "entry_id": "...", "booking_date": "YYYY-MM-DD",
///    "amount": "12.34", "currency": "EUR",
///    "creditor_name": "...", "remittance": "..." }`.
/// Provider tag preserved: `provider_txn_id` is prefixed `enable_banking:`.
pub fn map_transaction(t: &serde_json::Value) -> Option<RemoteTransaction> {
    let entry_id = t.get("entry_id")?.as_str()?.to_string();
    let date_str = t
        .get("booking_date")
        .and_then(|v| v.as_str())
        .or_else(|| t.get("value_date").and_then(|v| v.as_str()))?;
    let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    let amount: rust_decimal::Decimal = t
        .get("amount")
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.to_string().into())
        })
        .and_then(|s| s.trim_matches('"').parse().ok())?;
    let description = t
        .get("remittance")
        .and_then(|v| v.as_str())
        .or_else(|| t.get("description").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let payee = t
        .get("creditor_name")
        .and_then(|v| v.as_str())
        .or_else(|| t.get("debtor_name").and_then(|v| v.as_str()))
        .map(str::to_string);
    Some(RemoteTransaction {
        provider_txn_id: format!("enable_banking:{entry_id}"),
        date,
        amount,
        description,
        payee,
    })
}

#[async_trait]
impl Provider for EnableBankingProvider {
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError> {
        // Enable Banking uses an application-level JWT, not a
        // per-user public-token exchange. Return unchanged.
        Ok(public_token.to_string())
    }

    async fn fetch_transactions(
        &self,
        access_token: &str,
        _cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError> {
        // Test shape: the access token carries a JSON payload
        // `{ "transactions": [...] }` so the mapping is exercised
        // without live credentials.
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(access_token) {
            if let Some(arr) = payload.get("transactions").and_then(|v| v.as_array()) {
                let txns: Vec<RemoteTransaction> = arr.iter().filter_map(map_transaction).collect();
                return Ok((txns, None));
            }
        }
        Err(BankFeedError::MissingCredential(
            "ENABLE_BANKING_KEY_ID (live OAuth not configured; pass a JSON payload access_token in tests)".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_enable_banking_shape_with_provider_tag() {
        let t = serde_json::json!({
            "entry_id": "EB-1",
            "booking_date": "2026-08-21",
            "amount": "-42.50",
            "creditor_name": "ACME GmbH",
            "remittance": "Office supplies",
        });
        let r = map_transaction(&t).unwrap();
        assert_eq!(r.provider_txn_id, "enable_banking:EB-1");
        assert_eq!(
            r.date,
            chrono::NaiveDate::from_ymd_opt(2026, 8, 21).unwrap()
        );
        assert_eq!(r.amount, "-42.50".parse().unwrap());
        assert_eq!(r.payee.as_deref(), Some("ACME GmbH"));
        assert_eq!(r.description, "Office supplies");
    }

    #[test]
    fn rejects_unparsable_rows() {
        assert!(map_transaction(&serde_json::json!({})).is_none());
        assert!(map_transaction(&serde_json::json!({
            "entry_id": "x", "booking_date": "not-a-date", "amount": "1.00",
        }))
        .is_none());
    }
}
