//! HTTP integration tests for per-ledger-per-year transaction
//! numbering (`a5-transaction-numbering`).
//!
//! Covers:
//! - First txn of year is auto-numbered `YYYY-000001`.
//! - Year resets the counter.
//! - Ledger resets the counter (separate counters per ledger).
//! - User-supplied number is accepted and stored.
//! - Duplicate number → 400 / Validation error.
//! - Number appears in the list endpoint.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap(server: &TestServer) -> (String, Uuid, Uuid, Uuid) {
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
            ("email", "numbering-owner@example.com"),
            ("username", "numbering-owner"),
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
                ("email", "numbering-owner@example.com"),
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
            ("name", "Numbering Co"),
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

async fn post_txn(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    cash: Uuid,
    sales: Uuid,
    date: &str,
    description: &str,
    number: Option<&str>,
) -> reqwest::Response {
    let cash_s = cash.to_string();
    let sales_s = sales.to_string();
    let mut form: Vec<(&str, &str)> = vec![
        ("date", date),
        ("description", description),
        ("payee", ""),
        ("reference", ""),
        ("lines[0][account_id]", cash_s.as_str()),
        ("lines[0][direction]", "DEBIT"),
        ("lines[0][amount]", "10.00"),
        ("lines[0][memo]", ""),
        ("lines[1][account_id]", sales_s.as_str()),
        ("lines[1][direction]", "CREDIT"),
        ("lines[1][amount]", "10.00"),
        ("lines[1][memo]", ""),
    ];
    if let Some(n) = number {
        form.push(("number", n));
    }
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .form(&form)
        .send()
        .await
        .expect("POST transaction")
}

#[tokio::test]
async fn http_first_txn_of_year_is_000001() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;
    let resp = post_txn(&server, &cookie, ledger_id, cash, sales, "2026-08-15", "first", None).await;
    assert_eq!(resp.status(), 303);

    let pool = server.db().pool();
    let row: (Option<String>, Option<i32>) =
        sqlx::query_as("SELECT number, number_year FROM transactions WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0.as_deref(), Some("2026-000001"));
    assert_eq!(row.1, Some(2026));
}

#[tokio::test]
async fn http_year_resets_counter() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;
    let r1 = post_txn(&server, &cookie, ledger_id, cash, sales, "2025-12-30", "in 2025", None).await;
    assert_eq!(r1.status(), 303);
    let r2 = post_txn(&server, &cookie, ledger_id, cash, sales, "2026-01-02", "in 2026", None).await;
    assert_eq!(r2.status(), 303);

    let pool = server.db().pool();
    let rows: Vec<(Option<String>, Option<i32>)> = sqlx::query_as(
        "SELECT number, number_year FROM transactions
         WHERE ledger_id = $1 ORDER BY number_year ASC",
    )
    .bind(ledger_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows[0].0.as_deref(), Some("2025-000001"));
    assert_eq!(rows[0].1, Some(2025));
    assert_eq!(rows[1].0.as_deref(), Some("2026-000001"));
    assert_eq!(rows[1].1, Some(2026));
}

#[tokio::test]
async fn http_ledger_resets_counter() {
    let server = TestServer::new().await;
    let (cookie, ledger1, cash1, sales1) = bootstrap(&server).await;
    let resp = post_txn(&server, &cookie, ledger1, cash1, sales1, "2026-08-15", "ledger1 first", None).await;
    assert_eq!(resp.status(), 303);

    // Create a second ledger via the HTTP endpoint.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Numbering Co 2"),
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
    let ledger2 = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let pool = server.db().pool();
    let cash2: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
    )
    .bind(ledger2)
    .fetch_one(&pool)
    .await
    .unwrap();
    let sales2: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'",
    )
    .bind(ledger2)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = post_txn(&server, &cookie, ledger2, cash2, sales2, "2026-08-15", "ledger2 first", None).await;
    assert_eq!(resp.status(), 303);

    // Each ledger's first txn must be 2026-000001.
    let n1: Option<String> = sqlx::query_scalar(
        "SELECT number FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger1)
    .fetch_one(&pool)
    .await
    .unwrap();
    let n2: Option<String> = sqlx::query_scalar(
        "SELECT number FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n1.as_deref(), Some("2026-000001"));
    assert_eq!(n2.as_deref(), Some("2026-000001"));
}

#[tokio::test]
async fn http_manual_number_accepted() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;
    let resp = post_txn(
        &server,
        &cookie,
        ledger_id,
        cash,
        sales,
        "2026-08-15",
        "manual",
        Some("2026-EXPENSE-42"),
    )
    .await;
    assert_eq!(resp.status(), 303);

    let pool = server.db().pool();
    let row: (Option<String>,) = sqlx::query_as(
        "SELECT number FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0.as_deref(), Some("2026-EXPENSE-42"));

    // The next auto-numbered txn must skip past this one.
    let resp = post_txn(&server, &cookie, ledger_id, cash, sales, "2026-08-16", "auto", None).await;
    assert_eq!(resp.status(), 303);
    let n: Option<String> = sqlx::query_scalar(
        "SELECT number FROM transactions WHERE ledger_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // count was 1 (manual) + 1 (auto) = 2; auto-number = 2.
    assert_eq!(n.as_deref(), Some("2026-000002"));
}

#[tokio::test]
async fn http_duplicate_number_409() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;

    // First: success.
    let r1 = post_txn(
        &server,
        &cookie,
        ledger_id,
        cash,
        sales,
        "2026-08-15",
        "first",
        Some("2026-DUP"),
    )
    .await;
    assert_eq!(r1.status(), 303);

    // Second: same number → 409 Conflict.
    let r2 = post_txn(
        &server,
        &cookie,
        ledger_id,
        cash,
        sales,
        "2026-08-16",
        "second",
        Some("2026-DUP"),
    )
    .await;
    assert_eq!(
        r2.status(),
        409,
        "duplicate number must return 409 Conflict; got {}",
        r2.status()
    );
    let body = r2.text().await.unwrap_or_default();
    assert!(
        body.to_lowercase().contains("already used") || body.contains("2026-DUP"),
        "body must explain the duplicate; got {body}"
    );
}

#[tokio::test]
async fn http_number_in_list_view() {
    let server = TestServer::new().await;
    let (cookie, ledger_id, cash, sales) = bootstrap(&server).await;
    let resp = post_txn(
        &server,
        &cookie,
        ledger_id,
        cash,
        sales,
        "2026-08-15",
        "listed",
        None,
    )
    .await;
    assert_eq!(resp.status(), 303);

    // The GET /api/v1/ledgers/{id}/transactions response must
    // include the auto-generated number.
    let pool = server.db().pool();
    let token = openaccounting::auth::api_token::issue_token(
        &pool,
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email = 'numbering-owner@example.com'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "test-token",
    )
    .await
    .unwrap();

    let resp = server
        .client()
        .get(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token.plaintext),
        )
        .send()
        .await
        .expect("GET transactions");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["number"].as_str(),
        Some("2026-000001"),
        "list response must include the auto-generated number"
    );
}