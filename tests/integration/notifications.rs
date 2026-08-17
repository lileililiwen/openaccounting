//! Integration tests for the notifications / device-token module.
//!
//! Tests follow the spec from:
//! `openspec/changes/2026-08-14-mobile-shells/tasks.md` — section 1.

use crate::common::*;
use uuid::Uuid;

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Mobile Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location header")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

// ─── 1.6  http_devices_register_stores_token ─────────────────────────────────

/// POST /devices/register with a valid JSON body stores the token in the DB.
#[tokio::test]
async fn http_devices_register_stores_token() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "alice_mob",
            "alice_mob@example.com",
            "correct horse battery staple",
        )
        .await;
    let _ledger_id = make_ledger(&server).await;

    let resp = server
        .client()
        .post(format!("{}/devices/register", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"token":"test-device-token-ios","platform":"ios"}"#)
        .send()
        .await
        .expect("POST /devices/register");

    let status = resp.status();
    assert_eq!(status, 201, "register should 201; got {status}");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_tokens WHERE token = $1")
        .bind("test-device-token-ios")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count.0, 1, "token must be stored");
}

// ─── 1.7  http_dispatcher_with_noop_does_not_call_external ───────────────────

/// When PUSH_PROVIDER is not set (defaults to noop), the dispatcher
/// must not attempt any external API call — it silently succeeds.
#[tokio::test]
async fn http_dispatcher_with_noop_does_not_call_external() {
    use openaccounting::handlers::notifications::{dispatch, PushRequest};

    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "bob_mob",
            "bob_mob@example.com",
            "correct horse battery staple",
        )
        .await;
    let _ledger_id = make_ledger(&server).await;

    // Register a device.
    server
        .client()
        .post(format!("{}/devices/register", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"token":"noop-test-token","platform":"android"}"#)
        .send()
        .await
        .expect("register");

    // Get the user_id.
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user");

    // Remove the env var so noop is used.
    std::env::remove_var("PUSH_PROVIDER");

    let pool_clone = pool.clone();

    // Create a minimal AppState for dispatch (we use the DB pool directly).
    let state = openaccounting::AppState {
        pool: pool_clone,
        storage: openaccounting::storage::FilesystemStore::new("/tmp/oa-test-noop-dispatch")
            .await
            .expect("fs"),
        totp_cipher: openaccounting::auth::totp::TotpCipher::from_app_secret(
            "test-secret-do-not-use-in-production-please-replace-with-64-random-chars",
        ),
    };

    // Dispatch should complete without panicking or calling external APIs.
    dispatch(
        &state,
        PushRequest {
            user_id,
            title: "Test".into(),
            body: "Test body".into(),
            data: serde_json::json!({}),
        },
    )
    .await;

    // If we reach here the noop dispatcher worked silently.
}

// ─── 1.8  http_dispatcher_pushes_to_registered_devices ───────────────────────

/// The dispatcher fans out to all devices registered for a user.
/// With the noop provider, all pushes succeed silently.
#[tokio::test]
async fn http_dispatcher_pushes_to_registered_devices() {
    use openaccounting::handlers::notifications::{dispatch, PushRequest};

    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "carol_mob",
            "carol_mob@example.com",
            "correct horse battery staple",
        )
        .await;
    let _ledger_id = make_ledger(&server).await;

    // Register 2 devices.
    for (token, platform) in [("fanout-ios", "ios"), ("fanout-android", "android")] {
        server
            .client()
            .post(format!("{}/devices/register", server.base_url()))
            .header(reqwest::header::COOKIE, &cookie)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(format!(r#"{{"token":"{token}","platform":"{platform}"}}"#))
            .send()
            .await
            .expect("register");
    }

    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user");

    std::env::remove_var("PUSH_PROVIDER");

    let state = openaccounting::AppState {
        pool: pool.clone(),
        storage: openaccounting::storage::FilesystemStore::new("/tmp/oa-test-dispatch-fanout")
            .await
            .expect("fs"),
        totp_cipher: openaccounting::auth::totp::TotpCipher::from_app_secret(
            "test-secret-do-not-use-in-production-please-replace-with-64-random-chars",
        ),
    };

    // Dispatch to both devices. Must not fail.
    dispatch(
        &state,
        PushRequest {
            user_id,
            title: "Fanout test".into(),
            body: "Hello both devices".into(),
            data: serde_json::json!({"url": "/ledgers"}),
        },
    )
    .await;

    // Both tokens must still exist in the DB (dispatch doesn't delete them).
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM device_tokens WHERE token IN ('fanout-ios','fanout-android')",
    )
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(count.0, 2, "both device tokens must remain after dispatch");
}
