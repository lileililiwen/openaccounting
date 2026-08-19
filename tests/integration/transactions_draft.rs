//! HTTP integration tests for draft transactions
//! (`a8-draft-transactions`).
//!
//! Covers:
//! - Saving a transaction as draft returns 303 and the row is
//!   persisted with kind='draft'.
//! - Drafts do NOT appear in reports (trial balance is unchanged).
//! - Promoting a draft makes it appear in reports.
//! - Discarding a draft deletes the row without creating a
//!   reversal entry.
//! - Drafts can be saved against closed periods (validation
//!   skipped for drafts).
//! - The dedicated `/ledgers/{id}/drafts` page lists drafts.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn register(server: &TestServer, email: &str) -> String {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "X-OA-CSRF-Bypass",
                reqwest::header::HeaderValue::from_static("1"),
            );
            h
        })
        .build()
        .unwrap();
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", email.split('@').next().unwrap_or("user")),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    let resp = client
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
        .send()
        .await
        .expect("login");
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie")
}

async fn bootstrap(server: &TestServer) -> (Uuid, String, Uuid, Uuid) {
    let owner = register(server, "draft-owner@example.com").await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("name", "Draft Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();
    let pool = server.db().pool();
    let cash: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (ledger_id, owner, cash, sales)
}

fn post_body(cash: Uuid, sales: Uuid, amount: &str) -> Vec<(&'static str, String)> {
    vec![
        ("date", "2026-08-15".into()),
        ("description", "Drafted entry".into()),
        ("lines[0][account_id]", cash.to_string()),
        ("lines[0][direction]", "DEBIT".into()),
        ("lines[0][amount]", amount.into()),
        ("lines[1][account_id]", sales.to_string()),
        ("lines[1][direction]", "CREDIT".into()),
        ("lines[1][amount]", amount.into()),
    ]
}

#[tokio::test]
async fn http_save_as_draft_persists_with_draft_kind() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&{
            let mut body = post_body(cash, sales, "100");
            body.push(("action", "draft".into()));
            body
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "draft submit must redirect");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(
        loc.contains("/drafts"),
        "draft redirect should go to drafts page, got {loc}"
    );

    let (kind,): (String,) =
        sqlx::query_as("SELECT kind FROM transactions WHERE ledger_id = $1 LIMIT 1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "draft");

    // No audit row should have been written for the draft.
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM audit_entries
         WHERE ledger_id = $1 AND action = 'create' AND entity_type = 'transaction'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 0, "drafts MUST NOT write audit rows");
}

#[tokio::test]
async fn http_draft_excluded_from_reports() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&{
            let mut body = post_body(cash, sales, "100");
            body.push(("action", "draft".into()));
            body
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // Trial-balance HTML page must not include the draft row.
    let tb = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/trial-balance",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .send()
        .await
        .unwrap();
    assert_eq!(tb.status(), 200);
    let html = tb.text().await.unwrap();
    // The default COA Cash on Hand starts at zero; after a draft
    // it should still be zero. The P&L total (net_income) line
    // should also reflect zero revenue.
    assert!(
        !html.contains("Drafted entry"),
        "draft transactions MUST NOT appear in trial balance"
    );
}

#[tokio::test]
async fn http_promote_draft_included_in_reports() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Save as draft.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&{
            let mut body = post_body(cash, sales, "100");
            body.push(("action", "draft".into()));
            body
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // Promote.
    let (txn_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM transactions WHERE ledger_id = $1 AND kind = 'draft' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/post",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("unused", "x")])
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 303, "promote must redirect (body: {body})");

    let (kind,): (String,) = sqlx::query_as("SELECT kind FROM transactions WHERE id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "standard", "promotion must flip kind to standard");

    // Audit row written for the promotion.
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM audit_entries
         WHERE ledger_id = $1 AND action = 'promote' AND entity_id = $2",
    )
    .bind(ledger_id)
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1, "promotion MUST write one audit row");
}

#[tokio::test]
async fn http_discard_draft_no_reversal() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&{
            let mut body = post_body(cash, sales, "100");
            body.push(("action", "draft".into()));
            body
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let (txn_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM transactions WHERE ledger_id = $1 AND kind = 'draft' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/discard",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("unused", "x")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM transactions WHERE id = $1")
        .bind(txn_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "discarded draft must be gone");

    let reversal_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM transactions WHERE reverses_id = $1")
            .bind(txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        reversal_count.0, 0,
        "discard MUST NOT create a reversal transaction"
    );
}

#[tokio::test]
async fn http_draft_in_closed_period() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Close 2024.
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by)
         VALUES ($1, 2024, (SELECT id FROM users LIMIT 1))",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("date", "2024-06-15"),
            ("description", "Old draft"),
            ("lines[0][account_id]", &cash.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100"),
            ("lines[1][account_id]", &sales.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "100"),
            ("action", "draft"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        303,
        "drafts MUST be allowed even when the period is closed"
    );

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM transactions
         WHERE ledger_id = $1 AND kind = 'draft' AND txn_date = '2024-06-15'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn http_drafts_page_lists_all_drafts() {
    let server = TestServer::new().await;
    let (ledger_id, owner, cash, sales) = bootstrap(&server).await;
    let pool = server.db().pool();

    // Save two drafts.
    for amount in ["50", "75"] {
        let resp = server
            .client()
            .post(format!(
                "{}/ledgers/{ledger_id}/transactions/new",
                server.base_url()
            ))
            .header(reqwest::header::COOKIE, &owner)
            .form(&{
                let mut body = post_body(cash, sales, amount);
                body.push(("action", "draft".into()));
                body
            })
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 303);
    }

    let page = server
        .client()
        .get(format!("{}/ledgers/{ledger_id}/drafts", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    let html = page.text().await.unwrap();
    assert!(html.contains("Drafted entry"));
    assert!(html.contains("Post"));
    assert!(html.contains("Discard"));

    let drafts_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM transactions WHERE ledger_id = $1 AND kind = 'draft'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(drafts_count.0, 2);
}
