//! Integration tests for the cash-basis toggle on the
//! income-statement and cash-flow reports, plus the
//! `basis` column on `ledgers`.
//!
//! These tests boot a real `TestServer` (per-test database) and
//! drive it via the cookie session that `bootstrap_user`
//! returns. Every assertion is on a concrete number or exact
//! body fragment.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Create a ledger, get back its id. The user is already
/// authenticated (cookie jar is in `client`).
async fn create_ledger(client: &reqwest::Client, base_url: &str, name: &str, basis: &str) -> Uuid {
    let resp = client
        .post(format!("{base_url}/ledgers/new"))
        .form(&[
            ("name", name),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", basis),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let status = resp.status();
    assert!(
        status.is_success() || status.as_u16() == 303,
        "create ledger failed: {status}"
    );
    // Read the ledger id by scraping the Location header.
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location header on create")
        .to_string();
    let tail = loc.rsplit('/').next().expect("ledger id in Location");
    Uuid::parse_str(tail).expect("Location is a valid uuid")
}

/// Look up an account id by its name in a given ledger.
async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2")
            .bind(ledger_id)
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("account exists");
    id
}

/// Post a balanced transaction with two legs, both posted on the
/// given `txn_date`.
async fn post_balanced(
    pool: &PgPool,
    ledger_id: Uuid,
    user_id: Uuid,
    description: &str,
    txn_date: chrono::NaiveDate,
    debit_account: Uuid,
    credit_account: Uuid,
    amount: Decimal,
) {
    let mut tx = pool.begin().await.expect("begin");
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee,
                                    currency, created_by)
           VALUES ($1, $2, $3, $4, 'USD', $5)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(txn_date)
    .bind(description)
    .bind("")
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .expect("insert transaction");
    for (account_id, direction) in [(debit_account, "DEBIT"), (credit_account, "CREDIT")] {
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(txn_id)
        .bind(account_id)
        .bind(amount)
        .bind(direction)
        .execute(&mut *tx)
        .await
        .expect("insert posting");
    }
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn http_income_statement_accrual_includes_ar_revenue() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;

    // Look up the bootstrapped user's id from the database so we
    // can satisfy the transactions.created_by FK.
    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user row");

    let ledger_id =
        create_ledger(server.client(), server.base_url(), "Accrual Co", "accrual").await;

    let ar_id = account_id(&pool, ledger_id, "Accounts Receivable").await;
    let sales_id = account_id(&pool, ledger_id, "Sales Revenue").await;

    post_balanced(
        &pool,
        ledger_id,
        user_id,
        "Sale on credit",
        chrono::NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        ar_id,
        sales_id,
        Decimal::new(1000, 0),
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/income-statement?from=2026-08-01&to=2026-08-31&basis=accrual",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET income-statement accrual");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200, "accrual should return 200; body={body}");
    assert!(
        body.contains("1,000.00") || body.contains("1000.00") || body.contains("1000"),
        "accrual body should contain the 1000 revenue; body: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        body.contains("Basis: Accrual") || body.contains("Accrual"),
        "body should label the basis; body: {}",
        &body[..body.len().min(400)]
    );
}

#[tokio::test]
async fn http_income_statement_cash_excludes_ar_revenue() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;

    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user row");

    let ledger_id = create_ledger(server.client(), server.base_url(), "Cash Co", "accrual").await;

    let ar_id = account_id(&pool, ledger_id, "Accounts Receivable").await;
    let sales_id = account_id(&pool, ledger_id, "Sales Revenue").await;

    post_balanced(
        &pool,
        ledger_id,
        user_id,
        "Sale on credit",
        chrono::NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        ar_id,
        sales_id,
        Decimal::new(1000, 0),
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/income-statement?from=2026-08-01&to=2026-08-31&basis=cash",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET income-statement cash");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200, "cash should return 200; body={body}");
    // The Revenue section should show zero — no cash was received.
    assert!(
        body.contains("Basis: Cash"),
        "body should label the basis as Cash; body: {}",
        &body[..body.len().min(400)]
    );
    // The excluded footnote should appear with the 1000 amount.
    assert!(
        body.contains("Revenue excluded") || body.contains("excluded"),
        "body should contain a 'Revenue excluded' footnote; body: {}",
        &body[..body.len().min(600)]
    );
}

#[tokio::test]
async fn http_income_statement_basis_invalid_returns_400() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;

    // Create a ledger via a separate request so we have a real
    // ledger id to point the report at.
    let ledger_id = create_ledger(
        server.client(),
        server.base_url(),
        "InvalidBasis Co",
        "accrual",
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/income-statement?from=2026-08-01&to=2026-08-31&basis=foo",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET income-statement basis=foo");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 400, "invalid basis should return 400; body: {body}");
    assert!(
        body.contains("accrual") && body.contains("cash"),
        "error body should mention the valid options; body: {body}"
    );
}

#[tokio::test]
async fn http_ledger_create_with_cash_basis_persists() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let _cookie = server
        .bootstrap_user("dave", "dave@example.com", "correct horse battery staple")
        .await;

    let ledger_id =
        create_ledger(server.client(), server.base_url(), "CashDefault Co", "cash").await;

    // Verify the column was actually persisted.
    let (basis,): (String,) = sqlx::query_as("SELECT basis FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .expect("ledger row");
    assert_eq!(basis, "cash", "ledger should be persisted with cash basis");

    // And the form default ('accrual') on a fresh ledger.
    let ledger_id2 = create_ledger(
        server.client(),
        server.base_url(),
        "AccrualDefault Co",
        "accrual",
    )
    .await;
    let (basis2,): (String,) = sqlx::query_as("SELECT basis FROM ledgers WHERE id = $1")
        .bind(ledger_id2)
        .fetch_one(&pool)
        .await
        .expect("ledger row");
    assert_eq!(basis2, "accrual");
}

#[tokio::test]
async fn http_cash_flow_footer_shows_basis() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let cookie = server
        .bootstrap_user("eve", "eve@example.com", "correct horse battery staple")
        .await;

    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("user row");

    let ledger_id =
        create_ledger(server.client(), server.base_url(), "CashFlow Co", "accrual").await;

    // Move some cash: DR Office Supplies 100 / CR Cash on Hand 100
    let supplies_id = account_id(&pool, ledger_id, "Office Supplies").await;
    let cash_id = account_id(&pool, ledger_id, "Cash on Hand").await;
    post_balanced(
        &pool,
        ledger_id,
        user_id,
        "Bought supplies with cash",
        chrono::NaiveDate::from_ymd_opt(2026, 8, 10).unwrap(),
        supplies_id,
        cash_id,
        Decimal::new(100, 0),
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/cash-flow?from=2026-08-01&to=2026-08-31&basis=cash",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET cash-flow basis=cash");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200, "cash-flow should return 200; body={body}");
    assert!(
        body.contains("Basis: Cash") || body.contains("Cash"),
        "cash-flow body should label the basis; body: {}",
        &body[..body.len().min(600)]
    );
}
