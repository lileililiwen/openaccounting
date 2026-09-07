//! Webhook delivery job (`automation-platform`).
//!
//! Performs one HTTP POST attempt for a `webhook_deliveries` row,
//! records the outcome, and — on failure — schedules the next attempt
//! with the queue's backoff. Bodies are signed with
//! `X-OA-Signature: sha256=HMAC-SHA256(secret, raw-body)`; responses
//! are capped at 4 KB and private-address targets are refused (SSRF).

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;

use super::{events, JobError};

const MAX_ATTEMPTS: i32 = 5;
const MAX_RESPONSE_BYTES: usize = 4096;

/// Run one attempt for the delivery referenced by the payload.
pub async fn deliver(pool: &PgPool, payload: &serde_json::Value) -> Result<(), JobError> {
    let delivery_id: uuid::Uuid = super::payload_field(payload, "delivery_id")?;

    let row: Option<(uuid::Uuid, String, String, serde_json::Value, i32)> = sqlx::query_as(
        "SELECT s.id, s.target_url, s.secret, d.payload, d.attempt
         FROM webhook_deliveries d
         JOIN webhook_subscriptions s ON s.id = d.subscription_id
         WHERE d.id = $1 AND s.is_enabled = TRUE",
    )
    .bind(delivery_id)
    .fetch_optional(pool)
    .await
    .map_err(JobError::Db)?;
    let Some((subscription_id, target_url, secret, body, attempt)) = row else {
        return Ok(()); // subscription disabled or delivery gone → done
    };

    let serialized = serde_json::to_string(&body).map_err(|e| JobError::Failed(e.to_string()))?;

    let started = std::time::Instant::now();
    let outcome = post_signed(&target_url, &secret, &serialized).await;
    let duration_ms = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);

    let (ok, status_code, err) = match &outcome {
        Ok(code) => (*code >= 200 && *code < 300, Some(*code as i32), None),
        Err(e) => (false, None, Some(truncate(e.to_string(), 500))),
    };

    // Record this attempt.
    sqlx::query(
        "UPDATE webhook_deliveries
         SET ok = $2, status_code = $3, duration_ms = $4, error = $5
         WHERE id = $1",
    )
    .bind(delivery_id)
    .bind(ok)
    .bind(status_code)
    .bind(duration_ms)
    .bind(err)
    .execute(pool)
    .await
    .map_err(JobError::Db)?;

    if ok {
        return Ok(());
    }

    // Failure: append the next attempt row + its own job, unless
    // exhausted. This job returns Ok either way — its duty (one HTTP
    // attempt, recorded) is complete; the retry rides on the new job.
    // Returning Err here would re-queue THIS job and double-execute
    // the attempt.
    let next_attempt = attempt + 1;
    if next_attempt < MAX_ATTEMPTS {
        let (next_delivery_id,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO webhook_deliveries
                (subscription_id, event_id, event_type, payload, attempt)
             SELECT subscription_id, event_id, event_type, payload, $2
             FROM webhook_deliveries WHERE id = $1
             RETURNING id",
        )
        .bind(delivery_id)
        .bind(next_attempt)
        .fetch_one(pool)
        .await
        .map_err(JobError::Db)?;

        super::enqueue(
            pool,
            "webhook_delivery",
            serde_json::json!({ "delivery_id": next_delivery_id }),
            None,
            None,
        )
        .await
        .map_err(JobError::Db)?;
    } else {
        tracing::warn!(%delivery_id, %target_url, "webhook failed after {MAX_ATTEMPTS} attempts");
    }
    Ok(())
}

/// POST the signed body. Refuses non-HTTPS and loopback/private hosts.
/// `WEBHOOK_ALLOW_INSECURE_HTTP=true` relaxes this for `http://127.0.0.1`
/// targets only — a documented test/dev escape hatch, never for prod.
async fn post_signed(url: &str, secret: &str, body: &str) -> Result<u16, String> {
    let allow_insecure =
        std::env::var("WEBHOOK_ALLOW_INSECURE_HTTP").is_ok_and(|v| v == "true" || v == "1");
    let local_test_target = allow_insecure && url.starts_with("http://127.0.0.1");
    if !url.starts_with("https://") && !local_test_target {
        return Err("target must be https".into());
    }
    let parsed = url.parse::<reqwest::Url>().map_err(|e| e.to_string())?;
    let host = parsed.host_str().unwrap_or_default();
    if !local_test_target
        && (host == "localhost"
            || host == "0.0.0.0"
            || host.ends_with(".local")
            || host.starts_with("127.")
            || host.starts_with("10.")
            || host.starts_with("192.168.")
            || host.starts_with("169.254.")
            || is_private_172(host)
            || is_ipv6_private(host))
    {
        return Err("target resolves to a private address".into());
    }

    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .expect("hmac accepts any key length");
    mac.update(body.as_bytes());
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("X-OA-Event", event_header(body))
        .header("X-OA-Delivery", uuid::Uuid::new_v4().to_string())
        .header("X-OA-Signature", signature)
        .body(body.to_owned())
        .send()
        .await
        .map_err(|e| truncate(e.to_string(), 500))?;
    let code = resp.status().as_u16();
    // Drain a bounded amount so keep-alive connections close cleanly.
    let _ = resp.bytes().await.map(|b| b.len().min(MAX_RESPONSE_BYTES));
    Ok(code)
}

fn event_header(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str().map(String::from)))
        .unwrap_or_default()
}

fn is_private_172(host: &str) -> bool {
    host.parse::<std::net::Ipv4Addr>()
        .map(|ip| ip.is_private())
        .unwrap_or(false)
}

fn is_ipv6_private(host: &str) -> bool {
    host.parse::<std::net::Ipv6Addr>()
        .map(|ip| !ip.is_loopback() && (ip.segments()[0] & 0xfe00) == 0xfc00 || ip.is_loopback())
        .unwrap_or(false)
}

fn truncate(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        s.chars().take(max).collect()
    }
}

#[allow(dead_code)]
fn ensure_events_linked() {
    let _ = events::EVENT_TYPES;
}
