//! HTTP integration tests for the health and readiness probes
//! (`o5-health-endpoint`).
//!
//! Verifies:
//! - `/healthz` returns 200 anonymously, with the documented
//!   JSON body.
//! - `/readyz` returns 200 when Postgres + the documents
//!   directory are healthy.
//! - Both probes are reachable without a session cookie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

#[tokio::test]
async fn http_healthz_200() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .expect("GET /healthz");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "/healthz must return 200");
    let v: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(v["status"], "ok", "/healthz body must be {{status:ok}}");
}

#[tokio::test]
async fn http_readyz_200_when_healthy() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/readyz", server.base_url()))
        .send()
        .await
        .expect("GET /readyz");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "/readyz must return 200 when DB + disk are healthy; body={body}"
    );
    let v: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(v["status"], "ready");
}

#[tokio::test]
async fn http_healthz_no_auth_required() {
    // Use a fresh reqwest::Client with cookie_store OFF so
    // earlier cookies from other tests can't leak in.
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("anon client");

    let server = TestServer::new().await;
    let resp = anon
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .expect("GET /healthz anon");
    let status = resp.status();
    assert_eq!(status, 200, "/healthz must NOT require auth");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("\"ok\""),
        "body must include status: ok; got {body}"
    );
}

#[tokio::test]
async fn http_readyz_no_auth_required() {
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("anon client");

    let server = TestServer::new().await;
    let resp = anon
        .get(format!("{}/readyz", server.base_url()))
        .send()
        .await
        .expect("GET /readyz anon");
    let status = resp.status();
    // Must NOT be 303/307 (the login redirect).
    assert_ne!(
        status,
        reqwest::StatusCode::TEMPORARY_REDIRECT,
        "/readyz must NOT redirect to /login"
    );
    assert_ne!(
        status,
        reqwest::StatusCode::PERMANENT_REDIRECT,
        "/readyz must NOT redirect to /login"
    );
}
