//! HTTP integration tests for the session-timeout middleware
//! (`s6-session-timeout`).
//!
//! Two windows:
//! * Idle: 30 minutes since last request (configurable via
//!   `SESSION_IDLE_SECONDS`).
//! * Absolute: 12 hours since login (configurable via
//!   `SESSION_ABSOLUTE_SECONDS`).
//!
//! These tests don't wait 30 real minutes; they rebuild the
//! router with a 1-second idle window so the request after
//! the sleep fails fast.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::auth::session_timeout::SessionGuard;
use openaccounting::auth::totp::TotpCipher;
use openaccounting::{build_router_with_session_guard, AppConfig, AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

const APP_SECRET: &str = "test-secret-do-not-use-in-production-please-replace-with-64-random-chars";

async fn spawn_server_with_short_timeouts(
    server: &TestServer,
    idle: Duration,
    absolute: Duration,
) -> (String, tokio::task::JoinHandle<()>) {
    let pool = server.db().pool();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("oa-session-timeout-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    let storage = std::sync::Arc::new(
        openaccounting::storage::FilesystemStore::new(dir.join("documents"))
            .await
            .expect("storage"),
    );

    let state = AppState {
        pool,
        storage,
        totp_cipher: TotpCipher::from_app_secret(APP_SECRET),
    };
    let config = AppConfig::new(APP_SECRET).expect("app config");
    let guard = SessionGuard::new().idle(idle).absolute(absolute);
    let app = build_router_with_session_guard(state, config, guard);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    (url, handle)
}

async fn register_and_login(base_url: &str, client: &reqwest::Client) {
    let email = "alice@example.com";
    let pw = "X7!qZ4wN9pLk_3vR";
    client
        .post(format!("{base_url}/register"))
        .form(&[
            ("email", email),
            ("username", "alice"),
            ("password", pw),
            ("password_confirm", pw),
        ])
        .send()
        .await
        .expect("register");
    let resp = client
        .post(format!("{base_url}/login"))
        .form(&[("email", email), ("password", pw), ("next", "/")])
        .send()
        .await
        .expect("login");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "login must succeed; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_session_idle_timeout_redirects_to_login() {
    let server = TestServer::new().await;
    let (base_url, _handle) = spawn_server_with_short_timeouts(
        &server,
        Duration::from_secs(1),
        Duration::from_secs(3600),
    )
    .await;

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    register_and_login(&base_url, &client).await;

    // First authenticated request — must succeed.
    let resp = client
        .get(format!("{base_url}/account"))
        .send()
        .await
        .expect("GET /account");
    let status = resp.status();
    assert!(
        status.is_success() || status == 303,
        "first request after login must reach the handler; got {status}"
    );

    // Wait > idle (1 sec) and try again — middleware must redirect.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let resp = client
        .get(format!("{base_url}/account"))
        .send()
        .await
        .expect("GET /account after sleep");
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert_eq!(
        status, 303,
        "idle-expired request must redirect; got {status} (loc={loc})"
    );
    assert!(
        loc.contains("/login") && loc.contains("expired=1"),
        "redirect must point at /login with expired=1; got: {loc}"
    );
}

#[tokio::test]
async fn http_session_absolute_timeout_redirects_to_login() {
    let server = TestServer::new().await;
    let (base_url, _handle) = spawn_server_with_short_timeouts(
        &server,
        Duration::from_secs(3600),
        Duration::from_secs(1),
    )
    .await;

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    register_and_login(&base_url, &client).await;

    tokio::time::sleep(Duration::from_millis(1500)).await;
    let resp = client
        .get(format!("{base_url}/account"))
        .send()
        .await
        .expect("GET /account after sleep");
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert_eq!(
        status, 303,
        "absolute-expired request must redirect; got {status} (loc={loc})"
    );
    assert!(
        loc.contains("/login") && loc.contains("expired=1"),
        "redirect must point at /login with expired=1; got: {loc}"
    );
}

#[tokio::test]
async fn http_session_active_refresh_extends_lifetime() {
    let server = TestServer::new().await;
    let (base_url, _handle) =
        spawn_server_with_short_timeouts(&server, Duration::from_secs(3), Duration::from_secs(30))
            .await;

    let client = Arc::new(
        reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    );
    register_and_login(&base_url, &client).await;

    // Five polls, 1 sec apart. The session MUST stay alive.
    for i in 0..5 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let resp = client
            .get(format!("{base_url}/account"))
            .send()
            .await
            .expect("poll");
        let status = resp.status();
        let loc = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default();
        // Acceptable: 200 (handler ran) or 303 (redirect somewhere
        // benign — e.g. POST → GET). MUST NOT be a 303 to
        // /login?expired=1.
        if status == 303 {
            assert!(
                !loc.contains("expired=1"),
                "poll #{i} unexpectedly expired; loc={loc}"
            );
        }
    }
}

// Suppress unused warnings on the sync Mutex imports used by
// future test additions.
#[allow(dead_code)]
fn _suppress_unused(_: &Arc<()>) {}
