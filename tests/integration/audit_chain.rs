//! Tests for the cryptographic audit chain (`d1-audit-chain`).
//!
//! The `audit_entries` table now carries `prev_hash` / `hash` columns;
//! every `audit::log` appends a SHA-256 chained row. These tests
//! exercise:
//! - linking under the live server (each TestServer seeds a chain
//!   via its own migrations + `ensure_backfilled`);
//! - verification of an intact chain;
//! - detection of a tampered row (manual UPDATE breaking the chain);
//! - the admin verify endpoint (intact + broken).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

/// Insert a few audit rows via the application API, then verify the
/// chain is intact.
#[tokio::test]
async fn http_audit_verify_clean_200_ok() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let _cookie = server
        .bootstrap_user(
            "chain_alice",
            "chain_alice@example.com",
            "correct horse battery staple",
        )
        .await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Seed a handful of audit rows through the public function so
    // the chain-building code path runs.
    for i in 0..5 {
        openaccounting::audit::log(
            &pool,
            None,
            user_id,
            "seed",
            "test",
            None,
            None,
            Some(serde_json::json!({"n": i})),
        )
        .await
        .unwrap();
    }

    let breaks = openaccounting::audit::chain::verify(&pool).await.unwrap();
    assert!(
        breaks.is_empty(),
        "freshly-seeded chain must verify clean; breaks={breaks:?}"
    );

    let tail = openaccounting::audit::chain::latest_hash_hex(&pool)
        .await
        .unwrap()
        .expect("tail hash present");
    assert_eq!(tail.len(), 64, "SHA-256 hex is 64 chars");
}

/// Manually UPDATE one row's content and check the chain reports the
/// break point.
#[tokio::test]
async fn http_audit_verify_tampered_reports_break() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let _cookie = server
        .bootstrap_user(
            "chain_bob",
            "chain_bob@example.com",
            "correct horse battery staple",
        )
        .await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    for i in 0..3 {
        openaccounting::audit::log(
            &pool,
            None,
            user_id,
            "tamper-test",
            "test",
            None,
            None,
            Some(serde_json::json!({"n": i})),
        )
        .await
        .unwrap();
    }

    // Tamper with the middle row's new_value.
    sqlx::query("UPDATE audit_entries SET new_value = '{\"n\": 999}' WHERE action = 'tamper-test'")
        .execute(&pool)
        .await
        .unwrap();

    let breaks = openaccounting::audit::chain::verify(&pool).await.unwrap();
    assert!(!breaks.is_empty(), "tampered row must break the chain");
    assert!(
        breaks
            .iter()
            .any(|b| b.reason.contains("SHA256") || b.reason.contains("prev_hash")),
        "break reason must describe the hash mismatch; got {breaks:?}"
    );
}

/// The admin endpoint reports an intact chain for a normal user is
/// forbidden; for an admin it returns 200 with the status.
#[tokio::test]
async fn http_audit_verify_admin_endpoint() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let _cookie = server
        .bootstrap_user(
            "audit_admin",
            "audit_admin@example.com",
            "correct horse battery staple",
        )
        .await;

    // Make the user an admin (role column is lowercase).
    let (user_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE username = 'audit_admin'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    // A non-admin user must be forbidden.
    let cookie_user = server
        .bootstrap_user(
            "audit_plain",
            "audit_plain@example.com",
            "correct horse battery staple",
        )
        .await;
    let resp = server
        .client()
        .get(format!("{}/admin/audit/verify", server.base_url()))
        .header(reqwest::header::COOKIE, cookie_user)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::FORBIDDEN,
        "non-admin must be forbidden from /admin/audit/verify"
    );

    // The admin sees the page. The role is cached in the session at
    // login, so re-login with a fresh client after promoting the user.
    let admin_client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = admin_client
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", "audit_admin@example.com"),
            ("password", "correct horse battery staple"),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().as_u16() == 303 || resp.status().is_success(),
        "admin re-login failed: {}",
        resp.status()
    );
    let cookie = resp
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("admin session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let resp = admin_client
        .get(format!("{}/admin/audit/verify", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "/admin/audit/verify must return 200");
    assert!(
        body.contains("Chain intact") || body.contains("Chain BROKEN"),
        "page must state chain status; got: {body}"
    );
    assert!(
        body.contains("Audit chain"),
        "page must be the audit-chain page"
    );
}

/// The anchor worker appends a tail-hash line to the anchor file.
#[tokio::test]
async fn audit_anchor_writes_tail_hash_line() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let _cookie = server
        .bootstrap_user(
            "chain_carol",
            "chain_carol@example.com",
            "correct horse battery staple",
        )
        .await;
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    openaccounting::audit::log(&pool, None, user_id, "anchor", "test", None, None, None)
        .await
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anchor.log").to_string_lossy().into_owned();
    std::env::set_var("AUDIT_ANCHOR_FILE", &path);

    let hash = openaccounting::workers::audit_anchor::run_once(&pool)
        .await
        .unwrap()
        .expect("tail hash written");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(
        content.trim_end().ends_with(&hash),
        "anchor file must end with the tail hash; got: {content:?}"
    );
    assert_eq!(
        content.split_whitespace().count(),
        2,
        "one line: timestamp + hash"
    );

    // Clean up the env var so other tests aren't affected.
    std::env::remove_var("AUDIT_ANCHOR_FILE");
}
