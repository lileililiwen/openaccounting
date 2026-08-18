//! HTTP integration tests for dark-mode toggle (`u8-dark-mode`).
//!
//! Verifies:
//! - The default value for a freshly registered user is
//!   `"system"`.
//! - POSTing to `/account/theme` flips the persisted value.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

#[tokio::test]
async fn http_theme_default_is_system() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_dark",
            "alice_dark@example.com",
            "correct horse battery staple",
        )
        .await;

    let pool = server.db().pool();
    let (theme,): (String,) = sqlx::query_as("SELECT theme FROM users WHERE email = $1")
        .bind("alice_dark@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        theme, "system",
        "freshly registered user defaults to system"
    );

    // The auth/session response also carries the theme so the
    // layout can render the right toggle label.
    let resp = server
        .client()
        .get(format!("{}/account", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /account");
    let status = resp.status();
    assert_eq!(status, 200, "account page must render for authed user");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Toggle theme") || body.contains("◐"),
        "account page should expose the theme toggle"
    );
}

#[tokio::test]
async fn http_theme_toggle_persists() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_dark",
            "bob_dark@example.com",
            "correct horse battery staple",
        )
        .await;
    let pool = server.db().pool();

    // Toggle to dark.
    let resp = server
        .client()
        .post(format!("{}/account/theme", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("theme", "dark"), ("next", "/account")])
        .send()
        .await
        .expect("toggle to dark");
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "POST /account/theme must redirect; got {status}"
    );
    let (theme,): (String,) = sqlx::query_as("SELECT theme FROM users WHERE email = $1")
        .bind("bob_dark@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(theme, "dark", "persisted theme must be dark");

    // Toggle back to light.
    let resp = server
        .client()
        .post(format!("{}/account/theme", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("theme", "light"), ("next", "/account")])
        .send()
        .await
        .expect("toggle to light");
    assert!(
        resp.status() == 303 || resp.status() == 302,
        "second toggle must also redirect"
    );
    let (theme,): (String,) = sqlx::query_as("SELECT theme FROM users WHERE email = $1")
        .bind("bob_dark@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(theme, "light");

    // Invalid values are rejected with 400.
    let resp = server
        .client()
        .post(format!("{}/account/theme", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[("theme", "neon"), ("next", "/account")])
        .send()
        .await
        .expect("toggle to invalid");
    let (theme,): (String,) = sqlx::query_as("SELECT theme FROM users WHERE email = $1")
        .bind("bob_dark@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Theme must NOT have changed on a rejected request.
    assert_eq!(theme, "light", "invalid value must NOT mutate the row");
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "invalid value must 400/422; got {}",
        resp.status()
    );
}
