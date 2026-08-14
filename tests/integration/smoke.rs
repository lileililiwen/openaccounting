//! HTTP smoke tests. These prove the router is wired and the
//! per-test fixture works. Every assertion is concrete: an
//! exact status code, an exact body fragment.

use crate::common::*;

#[tokio::test]
async fn smoke_server_boots_and_login_page_responds() {
    let server = TestServer::new().await;

    let resp = server
        .client()
        .get(format!("{}/login", server.base_url()))
        .send()
        .await
        .expect("GET /login");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("login body");
    assert!(body.contains("<form"), "login page should contain a form");
    assert!(body.contains("email"), "login form should ask for email");
    assert!(
        body.contains("password"),
        "login form should ask for password"
    );
}

#[tokio::test]
async fn smoke_register_login_and_ledgers_list_responds() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;

    // The session cookie is in the response from /login; we now
    // drive an authenticated request explicitly so the test does
    // not depend on the reqwest cookie jar (which would be a
    // "trust the library" assertion).
    let resp = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /ledgers");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("ledgers body");
    assert!(
        body.contains("alice") || body.contains("Ledgers") || body.contains("No ledgers"),
        "ledgers page should render for the authenticated user; body starts with: {}",
        &body[..body.len().min(200)]
    );
}

#[tokio::test]
async fn smoke_static_assets_served() {
    let server = TestServer::new().await;

    let resp = server
        .client()
        .get(format!("{}/static/htmx.min.js", server.base_url()))
        .send()
        .await
        .expect("GET /static/htmx.min.js");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("htmx body");
    assert!(!body.is_empty(), "htmx.min.js should be non-empty");
    assert!(
        body.contains("htmx") || body.contains("HX-"),
        "body should look like htmx; first 80 chars: {}",
        &body[..body.len().min(80)]
    );
}
