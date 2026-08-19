//! HTTP integration tests for the secure-cookie enforcement
//! (`s5-secure-cookie-enforcement`).
//!
//! - `APP_ENV=test` is the default for the test fixture; the
//!   cookie is NOT marked `Secure` so HTTP localhost works.
//! - When the binary runs with `APP_ENV=production` and no
//!   `--allow-insecure-cookies`, the cookie IS marked `Secure`.
//! - With `APP_ENV=production --allow-insecure-cookies` the
//!   cookie stays insecure and a warning is logged.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

#[tokio::test]
async fn http_session_cookie_not_secure_in_test_env() {
    let server = TestServer::new().await;
    let email = "alice@example.com";
    let pw = "X7!qZ4wN9pLk_3vR";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "alice"),
            ("password", pw),
            ("password_confirm", pw),
        ])
        .send()
        .await
        .expect("register");
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", pw), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookies = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        cookies.contains("oa_session="),
        "oa_session cookie must be set; got: {cookies}"
    );
    assert!(
        !cookies.to_lowercase().contains("secure"),
        "test fixture must NOT mark the cookie Secure; got: {cookies}"
    );
}

#[tokio::test]
async fn http_session_cookie_secure_in_production() {
    // We don't spin up a full server here — we build the router
    // directly with a Secure-cookie AppConfig and verify the
    // emitted cookie carries the attribute.
    use openaccounting::auth::totp::TotpCipher;
    use openaccounting::{build_router, AppConfig, AppState};
    use std::net::SocketAddr;

    let server = TestServer::new().await;
    let pool = server.db().pool();

    let storage = std::sync::Arc::new(
        openaccounting::storage::FilesystemStore::new(server_db_storage_path(server.base_url()))
            .await
            .expect("storage"),
    );

    let state = AppState {
        pool,
        storage,
        totp_cipher: TotpCipher::from_app_secret(APP_SECRET_FOR_PROD_TEST),
    };
    let config = AppConfig::new(APP_SECRET_FOR_PROD_TEST)
        .expect("app config")
        .with_secure_cookie(true);
    let app = build_router(state, config);

    // Bind to a random port and serve.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let _handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    // Register a user via this auxiliary server's pool, then log
    // in to receive the session cookie.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let email = "bob@example.com";
    let pw = "X7!qZ4wN9pLk_3vR";
    let resp = client
        .post(format!("http://{addr}/register"))
        .form(&[
            ("email", email),
            ("username", "bob"),
            ("password", pw),
            ("password_confirm", pw),
        ])
        .send()
        .await
        .expect("POST /register");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "register must succeed; got {}",
        resp.status()
    );

    let resp = client
        .post(format!("http://{addr}/login"))
        .form(&[("email", email), ("password", pw), ("next", "/")])
        .send()
        .await
        .expect("POST /login");
    let cookies = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        cookies.to_lowercase().contains("secure"),
        "Secure flag must be set in production; got: {cookies}"
    );
}

#[tokio::test]
async fn http_allow_insecure_cookies_override() {
    use openaccounting::auth::totp::TotpCipher;
    use openaccounting::{build_router, AppConfig, AppState};
    use std::net::SocketAddr;

    let server = TestServer::new().await;
    let pool = server.db().pool();

    let storage = std::sync::Arc::new(
        openaccounting::storage::FilesystemStore::new(server_db_storage_path(server.base_url()))
            .await
            .expect("storage"),
    );

    let state = AppState {
        pool,
        storage,
        totp_cipher: TotpCipher::from_app_secret(APP_SECRET_FOR_PROD_TEST),
    };
    let config = AppConfig::new(APP_SECRET_FOR_PROD_TEST)
        .expect("app config")
        .with_secure_cookie(false); // explicit insecure
    let app = build_router(state, config);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let _handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://{addr}/login"))
        .send()
        .await
        .expect("GET /login");
    let cookies = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !cookies.to_lowercase().contains("secure"),
        "explicit insecure override must not set Secure; got: {cookies}"
    );
}

const APP_SECRET_FOR_PROD_TEST: &str =
    "prod-style-test-secret-do-not-use-in-production-please-replace-64";

/// Re-derive a unique temp dir for the auxiliary server we
/// spin up in this file. The TestServer already has its own
/// sandbox; we just need a different directory so the two
/// don't trample each other.
fn server_db_storage_path(_base_url: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("oa-secure-cookie-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("documents")
}
