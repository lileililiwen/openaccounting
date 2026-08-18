//! HTTP integration tests for configurable dashboard widgets
//! (`u5-dashboard-widgets`).
//!
//! Verifies that:
//! - The default layout renders the four default widgets.
//! - The widget picker persists a custom layout.
//! - The reset button restores the default.
//! - Unknown widget IDs are rejected with 400.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::handlers::dashboard_layout::{DEFAULT_LAYOUT, VOCABULARY};

/// Create a ledger via the HTTP endpoint so the default chart
/// of accounts is seeded.
async fn make_ledger(server: &TestServer, cookie: &str) -> uuid::Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", "Dash Test"),
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
        .expect("Location")
        .to_string();
    uuid::Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_dashboard_default_layout() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_dash",
            "alice_dash@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET dashboard");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "dashboard must render; got {status} body={body}"
    );

    // Every default widget must be present at least once.
    for w in DEFAULT_LAYOUT {
        assert!(
            body.contains(&format!("data-widget-id=\"{w}\"")),
            "dashboard must render widget {w}; body starts: {}",
            &body[..body.len().min(400)]
        );
    }

    // The Reset button is rendered.
    assert!(
        body.contains("/dashboard/reset") && body.contains("Reset to default"),
        "dashboard must offer Reset to default"
    );
}

#[tokio::test]
async fn http_dashboard_add_widget() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_dash",
            "bob_dash@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    // Build a custom layout that includes every vocabulary
    // widget — the test verifies that an extra widget
    // (`budget_burn`) added to the layout actually appears
    // in the rendered HTML.
    let custom: Vec<String> = VOCABULARY.iter().map(|s| s.to_string()).collect();
    let body = format!("widgets={}", custom.join(","));

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/dashboard/layout",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("set layout");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status == 303 || status == 302,
        "set layout must redirect; got {status} body={body}"
    );

    // DB row now stores the full vocabulary.
    let pool = server.db().pool();
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("bob_dash@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (widgets,): (Vec<String>,) = sqlx::query_as(
        "SELECT widgets FROM dashboard_layouts WHERE user_id = $1 AND ledger_id = $2",
    )
    .bind(uid)
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(widgets.len(), VOCABULARY.len());
    assert_eq!(widgets, custom);

    // Dashboard now renders every widget section.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET dashboard");
    let body = resp.text().await.unwrap();
    for w in VOCABULARY {
        assert!(
            body.contains(&format!("data-widget-id=\"{w}\"")),
            "dashboard must render widget {w}"
        );
    }
}

#[tokio::test]
async fn http_dashboard_reorder() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "carol_dash",
            "carol_dash@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    // Reverse the vocabulary to exercise a non-default order.
    let mut custom: Vec<String> = VOCABULARY.iter().map(|s| s.to_string()).collect();
    custom.reverse();
    let body = format!("widgets={}", custom.join(","));

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/dashboard/layout",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("set layout");
    assert!(resp.status() == 303 || resp.status() == 302);

    // Reload the dashboard; the first widget in the rendered
    // HTML must match the first widget in the layout.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET dashboard");
    let body = resp.text().await.unwrap();
    let expected_first = format!("data-widget-id=\"{}\"", custom[0]);
    let expected_last = format!("data-widget-id=\"{}\"", custom[custom.len() - 1]);
    let first_pos = body.find(&expected_first).expect("first widget missing");
    let last_pos = body.rfind(&expected_last).expect("last widget missing");
    assert!(
        first_pos < last_pos,
        "layout must be honoured: first={expected_first} last={expected_last}"
    );
}

#[tokio::test]
async fn http_dashboard_reset_default() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "dave_dash",
            "dave_dash@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    // First, save a non-default layout.
    let custom = ["charts".to_string(), "kpis".to_string()];
    let body = format!("widgets={}", custom.join(","));
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/dashboard/layout",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("set non-default");
    assert!(resp.status() == 303 || resp.status() == 302);

    // Then POST /dashboard/reset.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/dashboard/reset",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("reset");
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "reset must redirect; got {status}"
    );

    // The DB row is gone.
    let pool = server.db().pool();
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("dave_dash@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM dashboard_layouts WHERE user_id = $1 AND ledger_id = $2",
    )
    .bind(uid)
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 0, "reset must delete the layout row");

    // Dashboard renders the default layout again.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET dashboard");
    let body = resp.text().await.unwrap();
    for w in DEFAULT_LAYOUT {
        assert!(
            body.contains(&format!("data-widget-id=\"{w}\"")),
            "after reset, widget {w} must be present"
        );
    }
}

#[tokio::test]
async fn http_dashboard_rejects_unknown_widget() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "eve_dash",
            "eve_dash@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/dashboard/layout",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie.clone())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body("widgets=kpis,unicorn")
        .send()
        .await
        .expect("set bogus");
    let status = resp.status();
    assert!(
        status == 400 || status == 422,
        "unknown widget must 400/422; got {status}"
    );

    // The row must NOT have been written.
    let pool = server.db().pool();
    let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("eve_dash@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM dashboard_layouts WHERE user_id = $1 AND ledger_id = $2",
    )
    .bind(uid)
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 0, "rejected request must NOT mutate the row");
}
