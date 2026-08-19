//! HTTP integration tests for split-transaction UX
//! (`a4-split-transaction-ux`).
//!
//! The split button + JS live in the browser; we assert the
//! server contract: it accepts an arbitrary number of rows
//! that sum to zero.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap(server: &TestServer) -> (String, Uuid, Uuid, Uuid) {
    // Register + login with a fresh reqwest client so its
    // cookie jar is independent of the server's shared jar.
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
            ("email", "split-owner@example.com"),
            ("username", "split-owner"),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    let cookie = {
        let resp = client
            .post(format!("{}/login", server.base_url()))
            .form(&[
                ("email", "split-owner@example.com"),
                ("password", PASSWORD),
                ("next", "/"),
            ])
            .send()
            .await
            .unwrap();
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
            .unwrap()
    };

    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Split Co"),
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
    (cookie, ledger_id, cash, sales)
}

#[tokio::test]
async fn http_create_three_leg_split_succeeds() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;

    // A 3-leg split: 100 to Cash (DEBIT), 30 to Sales A (CREDIT),
    // 70 to Sales B (CREDIT). The user's split UI populates
    // rows; the server just receives a multi-leg form.
    let pool = server.db().pool();
    let sales_b: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'Sales B', 'INCOME', 'OPERATING_INCOME', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "3-leg split"),
            ("payee", ""),
            ("reference", ""),
            ("lines[0][account_id]", &cash.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[0][memo]", ""),
            ("lines[1][account_id]", &sales.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "30.00"),
            ("lines[1][memo]", ""),
            ("lines[2][account_id]", &sales_b.to_string()),
            ("lines[2][direction]", "CREDIT"),
            ("lines[2][amount]", "70.00"),
            ("lines[2][memo]", ""),
        ])
        .send()
        .await
        .expect("POST 3-leg split");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 303,
        "balanced 3-leg must succeed; got {status} body={body}"
    );
}

#[tokio::test]
async fn http_two_leg_with_auto_balance_value() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;

    // The classic 2-leg entry, with the auto-balanced amount
    // the JS would have filled in. Verifies the server accepts
    // the value the UI generates.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "Auto-balanced"),
            ("payee", ""),
            ("reference", ""),
            ("lines[0][account_id]", &cash.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "250.00"),
            ("lines[0][memo]", ""),
            ("lines[1][account_id]", &sales.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "250.00"),
            ("lines[1][memo]", ""),
        ])
        .send()
        .await
        .expect("POST auto-balanced");
    assert_eq!(resp.status(), 303);
}

#[tokio::test]
async fn http_single_row_rejected() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, _sales) = bootstrap(&server).await;

    // A single row cannot balance (PostingService requires ≥ 2).
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "Single row"),
            ("payee", ""),
            ("reference", ""),
            ("lines[0][account_id]", &cash.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[0][memo]", ""),
        ])
        .send()
        .await
        .expect("POST single row");
    // Single-row is re-rendered as 200 with the error message
    // embedded in the form template; the form page is returned.
    assert_eq!(
        resp.status(),
        200,
        "single-row returns the form with an error"
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.to_lowercase().contains("at least two")
            || body.to_lowercase().contains("do not balance"),
        "form must explain the rejection; got body: {body}"
    );
}
