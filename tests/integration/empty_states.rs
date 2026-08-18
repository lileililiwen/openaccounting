//! HTTP integration tests for empty-state rendering
//! (`u9-empty-states`).
//!
//! Verifies that:
//! - An empty transactions list renders the empty-state partial.
//! - An empty accounts list renders the empty-state partial.
//! - Each partial contains an inline SVG (no external resources)
//!   and the documented CTA link.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

/// Create a ledger via the public HTTP endpoint and return its id.
/// Uses `/ledgers/new` (which seeds default accounts).
async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Empty Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    assert!(
        status.is_success() || status.as_u16() == 303,
        "ledger create status={status} loc={loc:?}"
    );
    let loc = loc.expect("Location header");
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

#[tokio::test]
async fn http_empty_transactions_renders_partial() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_empty",
            "alice_empty@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/transactions",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /transactions");
    assert_eq!(resp.status(), 200, "transactions list must render");
    let body = resp.text().await.unwrap();

    // Empty-state partial rendered with the transactions marker.
    assert!(
        body.contains(r#"data-empty-state="transactions""#),
        "expected empty-state partial for transactions; body starts with: {}",
        &body[..body.len().min(400)]
    );
    // Inline SVG present (no JS, no external image).
    assert!(
        body.contains("<svg"),
        "empty state must contain an inline <svg> element"
    );
    assert!(
        body.contains("Record your first transaction"),
        "empty state must show the primary CTA text"
    );
    assert!(
        body.contains(&format!("/ledgers/{}/transactions/new", ledger_id)),
        "empty state CTA must link to the create-transaction page"
    );
}

#[tokio::test]
async fn http_empty_accounts_renders_partial() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "bob_empty",
            "bob_empty@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server).await;

    // A freshly-created ledger is auto-seeded with a default
    // chart of accounts, so the empty state would not normally
    // render. Clear the accounts table for this ledger so the
    // empty-state partial is what the user sees.
    sqlx::query("DELETE FROM accounts WHERE ledger_id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .expect("clear accounts");

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/accounts",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(resp.status(), 200, "accounts list must render");
    let body = resp.text().await.unwrap();

    assert!(
        body.contains(r#"data-empty-state="accounts""#),
        "expected empty-state partial for accounts; body starts with: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        body.contains("<svg"),
        "accounts empty state must contain an inline <svg> element"
    );
    assert!(
        body.contains("Create your first account"),
        "accounts empty state must show the primary CTA text"
    );
    assert!(
        body.contains(&format!("/ledgers/{}/accounts/new", ledger_id)),
        "accounts empty state CTA must link to the create-account page"
    );
}
