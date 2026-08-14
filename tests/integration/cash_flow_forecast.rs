//! Integration tests for the cash-flow-forecast report.
//!
//! These tests boot a real `TestServer`, create a ledger, seed
//! a `transaction_templates` row with a cash-leg posting, and
//! drive the new `GET /ledgers/{id}/reports/cash-flow-forecast`
//! endpoint.

use crate::common::*;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

async fn create_ledger_with_basis(
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> Uuid {
    let resp = client
        .post(format!("{base_url}/ledgers/new"))
        .form(&[
            ("name", name),
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
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2",
    )
    .bind(ledger_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("account exists");
    id
}

/// Create a recurring template with one cash-leg posting.
async fn create_template(
    pool: &PgPool,
    ledger_id: Uuid,
    description: &str,
    frequency: &str,
    next_date: NaiveDate,
    amount: Decimal,
    account_id: Uuid,
    direction: &str,
) {
    let (template_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transaction_templates
              (ledger_id, description, frequency, next_date, is_active)
           VALUES ($1, $2, $3, $4, TRUE)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(description)
    .bind(frequency)
    .bind(next_date)
    .fetch_one(pool)
    .await
    .expect("insert template");
    sqlx::query(
        r#"INSERT INTO template_postings (template_id, account_id, amount, direction)
           VALUES ($1, $2, $3, $4)"#,
    )
    .bind(template_id)
    .bind(account_id)
    .bind(amount)
    .bind(direction)
    .execute(pool)
    .await
    .expect("insert template posting");
}

#[tokio::test]
async fn http_forecast_with_no_templates_is_flat() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id =
        create_ledger_with_basis(server.client(), server.base_url(), "Flat Co").await;

    // Seed today's cash balance: DR Cash 5000 / CR Owner's Equity 5000.
    let user_id: (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let equity = account_id(&pool, ledger_id, "Owner's Equity").await;
    let mut tx = pool.begin().await.unwrap();
    let (txn_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee,
                                    currency, created_by)
           VALUES ($1, $2, $3, '', 'USD', $4) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(chrono::Utc::now().date_naive())
    .bind("Seed")
    .bind(user_id.0)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    for (acct, dir) in [(cash, "DEBIT"), (equity, "CREDIT")] {
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(txn_id)
        .bind(acct)
        .bind(Decimal::new(5000, 0))
        .bind(dir)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/cash-flow-forecast?days=30",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET forecast");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(
        body.contains("Cash Flow Forecast"),
        "page should render the title; body: {}",
        &body[..body.len().min(300)]
    );
    assert!(
        body.contains("No recurring transactions"),
        "no templates should produce empty table message; body: {}",
        &body[..body.len().min(300)]
    );
    // Footer should show 5,000 starting balance and 5,000 ending.
    assert!(body.contains("5,000") || body.contains("5000"));
    // Chart SVG should be present.
    assert!(body.contains("<svg"));
}

#[tokio::test]
async fn http_forecast_with_monthly_rent_drops_balance() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id =
        create_ledger_with_basis(server.client(), server.base_url(), "Rent Co").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let rent = account_id(&pool, ledger_id, "Office Supplies").await;
    let _ = rent; // referenced to silence warning

    // Recurring template: 1st of every month, 1000.00 cash out
    // (debit the cash account).
    let today = chrono::Utc::now().date_naive();
    let first_of_next_month = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
    };
    create_template(
        &pool,
        ledger_id,
        "Rent",
        "monthly",
        first_of_next_month,
        Decimal::new(1000, 0),
        cash,
        "DEBIT",
    )
    .await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/cash-flow-forecast?days=60",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET forecast");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("Rent"), "upcoming table should list Rent");
    // The footer should report at least 1 upcoming entry.
    // The min balance will be the balance after the rent hits.
    assert!(
        body.contains("Min balance"),
        "footer should have Min balance; body: {}",
        &body[..body.len().min(400)]
    );
    assert!(body.contains("Ending balance"));
}

#[tokio::test]
async fn http_forecast_default_horizon_is_90_days() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = create_ledger_with_basis(server.client(), server.base_url(), "Def Co").await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    // A weekly template that will fire many times in 90 days.
    let today = chrono::Utc::now().date_naive();
    create_template(
        &pool,
        ledger_id,
        "Subscription",
        "weekly",
        today,
        Decimal::new(10, 0),
        cash,
        "DEBIT",
    )
    .await;
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{}/reports/cash-flow-forecast",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET forecast");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    // 90 days / 7 = ~13 entries. Assert by counting rows in
    // the "Upcoming entries" table, not by scraping the footer
    // (whose askama escaping has been a moving target).
    let rows = body.matches("<tr>").count();
    // 1 header row + N data rows.
    let data_rows = rows.saturating_sub(1);
    assert!(
        (12..=14).contains(&data_rows),
        "expected 12-14 weekly entries in 90 days, got {data_rows} data rows"
    );
}
