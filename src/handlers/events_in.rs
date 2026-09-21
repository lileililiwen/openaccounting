//! Signed incoming event intake (`openapi-sdk`).
//!
//! `POST /api/events/{ledger_id}` accepts an allowlisted
//! `external.*` event from an external system. The request body is
//! verified against the ledger's `incoming_events_secret` with the
//! same HMAC scheme used for outgoing webhook signatures
//! (`X-OA-Event-Signature: sha256=<hex>`), then fanned out to the
//! automation rule engine on the jobs queue. Fail-closed: no secret,
//! no/invalid signature, or unknown event type is rejected.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    jobs::automation,
    AppState,
};

pub async fn intake(
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Response> {
    let secret: Option<String> =
        sqlx::query_scalar("SELECT incoming_events_secret FROM ledgers WHERE id = $1")
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some(secret) = secret.filter(|s| !s.is_empty()) else {
        // No secret configured: intake is disabled for this ledger.
        return Err(AppError::NotFound);
    };

    let provided = headers
        .get("X-OA-Event-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !verify_signature(&body, provided, &secret) {
        return Ok((StatusCode::UNAUTHORIZED, "bad signature").into_response());
    }

    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("body must be valid JSON".into()))?;
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !automation::INBOUND_EVENT_TYPES.contains(&event_type) {
        return Err(AppError::Validation(format!(
            "event type '{event_type}' is not an allowlisted incoming type"
        )));
    }
    let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);

    let queued = automation::match_and_enqueue(&state.pool, ledger_id, event_type, &data).await;
    Ok((
        StatusCode::ACCEPTED,
        axum::Json(serde_json::json!({
            "queued": queued,
            "event_type": event_type,
        })),
    )
        .into_response())
}

/// Verify `sha256=<hex>` HMAC-SHA256 over the raw body (same scheme
/// as outgoing webhook signatures). Constant-time compare, fail closed.
pub fn verify_signature(body: &[u8], header: &str, secret: &str) -> bool {
    let Some(mac_hex) = header.trim().strip_prefix("sha256=") else {
        return false;
    };
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    let Ok(provided) = hex::decode(mac_hex.trim()) else {
        return false;
    };
    if provided.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in provided.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Generate a fresh incoming-events secret for a ledger (`oa_in_…`).
pub fn generate_incoming_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("oa_in_{}", hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_roundtrip_and_rejection() {
        let secret = "oa_in_testsecret";
        let body = br#"{"type":"external.sync"}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        assert!(verify_signature(body, &sig, secret));
        assert!(!verify_signature(body, "sha256=deadbeef", secret));
        assert!(!verify_signature(body, "nope", secret));
        assert!(!verify_signature(b"tampered", &sig, secret));
    }
}
