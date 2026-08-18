//! HTTP integration tests for keyboard shortcuts
//! (`u1-keyboard-shortcuts`).
//!
//! The shortcuts themselves run in the browser and cannot be
//! exercised by the integration harness; what we verify here
//! is the wiring:
//!
//! * `static/js/shortcuts.js` is served.
//! * Every authenticated page includes the help overlay
//!   markup.
//! * `/login` and `/register` opt out via
//!   `data-shortcuts-off="true"` on `<body>` so the JS handler
//!   never installs listeners.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

#[tokio::test]
async fn http_shortcuts_js_is_served() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/js/shortcuts.js", server.base_url()))
        .send()
        .await
        .expect("GET shortcuts.js");
    let status = resp.status();
    assert_eq!(status, 200, "shortcuts.js must be served");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("navigate('/ledgers/:id/transactions')"),
        "shortcuts.js must contain the g+t binding; first 200 chars: {}",
        &body[..body.len().min(200)]
    );
    assert!(
        body.contains("'?'") && body.contains("Escape"),
        "shortcuts.js must handle '?' and 'Escape'"
    );
    assert!(
        body.contains("isTypingTarget"),
        "shortcuts.js must suppress typing targets"
    );
}

#[tokio::test]
async fn http_shortcut_overlay_on_protected_pages() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_kb",
            "alice_kb@example.com",
            "correct horse battery staple",
        )
        .await;

    // Pick a page that's available to a fresh user.
    let resp = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /ledgers");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("id=\"shortcuts-help\""),
        "help overlay markup must be present on protected pages; body starts: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        body.contains("src=\"/static/js/shortcuts.js\""),
        "shortcuts.js must be loaded"
    );
    // Body must NOT carry the opt-out attribute here.
    assert!(
        !body.contains(r#"data-shortcuts-off="true""#),
        "protected pages must not opt out of shortcuts"
    );
}

#[tokio::test]
async fn http_shortcut_disabled_on_auth_pages() {
    let server = TestServer::new().await;

    let resp = server
        .client()
        .get(format!("{}/login", server.base_url()))
        .send()
        .await
        .expect("GET /login");
    let status = resp.status();
    assert_eq!(status, 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains(r#"data-shortcuts-off="true""#),
        "/login must opt out of shortcuts via data-shortcuts-off"
    );
    assert!(
        body.contains("id=\"shortcuts-help\""),
        "the overlay markup may still be present; only the handler is disabled"
    );

    let resp = server
        .client()
        .get(format!("{}/register", server.base_url()))
        .send()
        .await
        .expect("GET /register");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains(r#"data-shortcuts-off="true""#),
        "/register must opt out of shortcuts"
    );
}

#[tokio::test]
async fn http_shortcut_help_lists_every_binding() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_kb",
            "bob_kb@example.com",
            "correct horse battery staple",
        )
        .await;
    let resp = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /ledgers");
    let body = resp.text().await.unwrap();
    // Every binding documented in the spec must be listed.
    for label in [
        "Ledgers",
        "Transactions",
        "Accounts",
        "Reports",
        "Create",
        "Esc",
    ] {
        assert!(body.contains(label), "help overlay must list {label}");
    }
}
