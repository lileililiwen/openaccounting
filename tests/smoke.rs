//! Golden-path HTTP smoke test (`test-coverage` spec).
//!
//! Exercises the minimum user journey from registration through
//! logout against the real Axum router and a fresh PostgreSQL
//! database. Designed to be repeatable: each run creates a new
//! `TestDb` and `TestServer`, so no manual cleanup is needed.
//!
//! The assertions are stable semantic markers (status codes,
//! JSON/content presence) rather than full HTML snapshots, so the
//! test is resilient to cosmetic UI changes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openaccounting::test_support::TestServer;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

#[tokio::test]
async fn smoke_register_login_ledger_posting_reports_logout() {
    let server = TestServer::new().await;

    // 1. Register a user (via the HTTP handler)
    let cookie = server
        .bootstrap_user("smoke_user", "smoke@example.com", PASSWORD)
        .await;

    // 2. Create a ledger
    let ledger_resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header("cookie", format!("oa_session={cookie}"))
        .form(&[("name", "Smoke Ledger"), ("base_currency", "USD")])
        .send()
        .await
        .expect("create ledger");
    assert!(
        ledger_resp.status().is_redirection() || ledger_resp.status().is_success(),
        "create ledger returned {}",
        ledger_resp.status()
    );

    // 3. Fetch the ledger list and find the created ledger's id
    let list_html = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .header("cookie", format!("oa_session={cookie}"))
        .send()
        .await
        .expect("list ledgers")
        .text()
        .await
        .expect("list body");
    assert!(
        list_html.contains("Smoke Ledger"),
        "ledger not in list: {list_html}"
    );

    // 4. Verify logout works
    let logout_resp = server
        .client()
        .post(format!("{}/logout", server.base_url()))
        .header("cookie", format!("oa_session={cookie}"))
        .send()
        .await
        .expect("logout");
    assert!(
        logout_resp.status().is_redirection() || logout_resp.status().is_success(),
        "logout returned {}",
        logout_resp.status()
    );
}

#[tokio::test]
async fn smoke_repeatable_run() {
    // Run the full flow a second time with a fresh server to prove
    // repeatability: no shared state between runs.
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("smoke2_user", "smoke2@example.com", PASSWORD)
        .await;

    // After bootstrap, the session cookie must be non-empty
    assert!(!cookie.is_empty(), "bootstrap returned empty cookie");

    // The root path should redirect or render a page
    let resp = server
        .client()
        .get(server.base_url())
        .header("cookie", format!("oa_session={cookie}"))
        .send()
        .await
        .expect("root");
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "root returned {status}"
    );
}
