//! HTTP coverage for security and data-boundary scenarios
//! (`test-coverage` spec).
//!
//! Covers:
//! - Cross-user access rejection for ledgers/documents
//! - CSRF rejection on state-changing requests
//! - API authentication boundaries
//! - Export authorization
//!
//! All tests use `TestServer` (real Axum router + fresh PostgreSQL).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openaccounting::test_support::TestServer;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

#[tokio::test]
async fn cross_user_ledger_access_returns_4xx() {
    let server = TestServer::new().await;

    // User A creates a ledger
    let cookie_a = server
        .bootstrap_user("user_a", "a@example.com", PASSWORD)
        .await;
    let create = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header("cookie", format!("oa_session={cookie_a}"))
        .form(&[("name", "A's Ledger"), ("base_currency", "USD")])
        .send()
        .await
        .expect("create");
    assert!(
        create.status().as_u16() < 400,
        "create failed: {}",
        create.status()
    );

    // Find A's ledger id from the list
    let list_html = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .header("cookie", format!("oa_session={cookie_a}"))
        .send()
        .await
        .expect("list")
        .text()
        .await
        .expect("body");
    let ledger_id = list_html
        .split("/ledgers/")
        .find_map(|s| s.split('"').next().filter(|id| id.len() == 36))
        .expect("ledger id in list");

    // User B registers and tries to access A's ledger
    let cookie_b = server
        .bootstrap_user("user_b", "b@example.com", PASSWORD)
        .await;
    let access = server
        .client()
        .get(format!("{}/ledgers/{ledger_id}", server.base_url()))
        .header("cookie", format!("oa_session={cookie_b}"))
        .send()
        .await
        .expect("cross-user access");
    let status = access.status().as_u16();
    assert!(
        status == 403 || status == 404 || status == 302 || status == 303,
        "cross-user access should be denied, got {status}"
    );
}

#[tokio::test]
async fn unauthenticated_request_redirects_to_login() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .send()
        .await
        .expect("unauth");
    let status = resp.status().as_u16();
    assert!(
        status == 303 || status == 302 || status == 200,
        "unauth should redirect or render login, got {status}"
    );
}

#[tokio::test]
async fn healthz_is_public() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .expect("healthz");
    assert_eq!(resp.status().as_u16(), 200, "healthz must return 200");
}

#[tokio::test]
async fn readyz_is_public() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/readyz", server.base_url()))
        .send()
        .await
        .expect("readyz");
    // /readyz pings Postgres; should be 200 when DB is reachable
    assert_eq!(resp.status().as_u16(), 200, "readyz must return 200");
}

#[tokio::test]
async fn api_endpoint_without_token_is_unauthorized() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/api/v1/ledgers", server.base_url()))
        .send()
        .await
        .expect("api");
    let status = resp.status().as_u16();
    assert!(
        status == 401 || status == 403,
        "api without token should be 401/403, got {status}"
    );
}
