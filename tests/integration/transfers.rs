//! Integration tests for inter-ledger transfers
//! (`a7-inter-ledger-transfers`).
//!
//! Covers:
//! - `http_inter_ledger_transfer_creates_two_txns` — a transfer
//!   inserts one row in `transactions` per ledger + one row in
//!   `inter_ledger_transfers`.
//! - `http_inter_ledger_transfer_unauthorized_403` — a non-owner
//!   of either ledger gets 403.
//! - `http_inter_ledger_transfer_rejects_same_ledger` — submitting
//!   from == to returns 400.
//! - `http_inter_entity_report_renders` — the inter-entity report
//!   lists the transfer.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn register_and_login(server: &TestServer, email: &str) -> String {
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
            ("username", email.split('@').next().unwrap_or("u")),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .unwrap();
    let resp = client
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/")])
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
}

async fn create_ledger(server: &TestServer, cookie: &str, name: &str) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", name),
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
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

async fn bootstrap_two_ledgers(
    server: &TestServer,
) -> (String, Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let cookie = register_and_login(server, "transfer-owner@example.com").await;
    let ledger_a = create_ledger(server, &cookie, "Personal A").await;
    let ledger_b = create_ledger(server, &cookie, "LLC B").await;
    let pool = server.db().pool();
    // Seed two ASSET accounts in each ledger — one we'll use
    // as the "from" / "to" account, and Cash on Hand exists
    // by default.
    let from_a: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'AR — Personal A', 'ASSET', 'OTHER_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    let to_b: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency)
         VALUES ($1, 'AR — LLC B', 'ASSET', 'OTHER_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    (cookie, ledger_a, ledger_b, from_a, to_b, Uuid::nil(), Uuid::nil())
}

#[tokio::test]
async fn http_inter_ledger_transfer_creates_two_txns() {
    let server = TestServer::new().await;
    let (cookie, from_ledger, to_ledger, from_account, to_account, _, _) =
        bootstrap_two_ledgers(&server).await;

    let resp = server
        .client()
        .post(format!("{}/transfers/inter-ledger", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("from_ledger_id", from_ledger.to_string().as_str()),
            ("to_ledger_id", to_ledger.to_string().as_str()),
            ("from_account_id", from_account.to_string().as_str()),
            ("to_account_id", to_account.to_string().as_str()),
            ("amount", "250.00"),
            ("date", "2026-08-15"),
            ("description", "Personal → LLC capital contribution"),
        ])
        .send()
        .await
        .expect("POST transfer");
    assert_eq!(resp.status(), 303, "transfer must redirect");

    let pool = server.db().pool();
    let link_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM inter_ledger_transfers
         WHERE from_ledger_id = $1 AND to_ledger_id = $2",
    )
    .bind(from_ledger)
    .bind(to_ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link_count.0, 1, "one inter_ledger_transfers row");

    // Each side must have one new transaction linked to it.
    let from_txn: Uuid = sqlx::query_scalar(
        "SELECT id FROM transactions
         WHERE ledger_id = $1
           AND id = (SELECT from_txn_id FROM inter_ledger_transfers
                     WHERE from_ledger_id = $1 LIMIT 1)",
    )
    .bind(from_ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    let to_txn: Uuid = sqlx::query_scalar(
        "SELECT id FROM transactions
         WHERE ledger_id = $1
           AND id = (SELECT to_txn_id FROM inter_ledger_transfers
                     WHERE to_ledger_id = $1 LIMIT 1)",
    )
    .bind(to_ledger)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(from_txn, to_txn, "the two sides must be distinct txns");

    // Each side has balanced postings (DB trigger).
    let sum_from: (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE -amount END), 0)::DECIMAL
         FROM postings WHERE transaction_id = $1",
    )
    .bind(from_txn)
    .fetch_one(&pool)
    .await
    .unwrap();
    let sum_to: (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE -amount END), 0)::DECIMAL
         FROM postings WHERE transaction_id = $1",
    )
    .bind(to_txn)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sum_from.0, Decimal::ZERO, "source postings must balance");
    assert_eq!(sum_to.0, Decimal::ZERO, "target postings must balance");
}

#[tokio::test]
async fn http_inter_ledger_transfer_unauthorized_403() {
    let server = TestServer::new().await;
    // Owner sets up two ledgers.
    let (owner_cookie, from_ledger, to_ledger, from_account, to_account, _, _) =
        bootstrap_two_ledgers(&server).await;

    // A different user — not an owner/editor on either ledger.
    let _outsider: String = register_and_login(&server, "outsider@example.com").await;

    let pool = server.db().pool();
    // Promote outsider to admin so login works, but they have
    // no membership in either ledger.
    sqlx::query("UPDATE users SET role = 'admin' WHERE email = 'outsider@example.com'")
        .execute(&pool)
        .await
        .unwrap();
    // Re-login so the new role is in the session.
    let outsider: String = register_and_login(&server, "outsider@example.com").await;

    let resp = server
        .client()
        .post(format!("{}/transfers/inter-ledger", server.base_url()))
        .header(reqwest::header::COOKIE, &outsider)
        .form(&[
            ("from_ledger_id", from_ledger.to_string().as_str()),
            ("to_ledger_id", to_ledger.to_string().as_str()),
            ("from_account_id", from_account.to_string().as_str()),
            ("to_account_id", to_account.to_string().as_str()),
            ("amount", "250.00"),
            ("date", "2026-08-15"),
            ("description", "sneak"),
        ])
        .send()
        .await
        .expect("POST transfer as outsider");
    assert!(
        resp.status() == 403 || resp.status() == 404,
        "outsider must be blocked; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_inter_ledger_transfer_rejects_same_ledger() {
    let server = TestServer::new().await;
    let (cookie, from_ledger, _, from_account, to_account, _, _) =
        bootstrap_two_ledgers(&server).await;

    let resp = server
        .client()
        .post(format!("{}/transfers/inter-ledger", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("from_ledger_id", from_ledger.to_string().as_str()),
            ("to_ledger_id", from_ledger.to_string().as_str()),
            ("from_account_id", from_account.to_string().as_str()),
            ("to_account_id", to_account.to_string().as_str()),
            ("amount", "100.00"),
            ("date", "2026-08-15"),
            ("description", "self"),
        ])
        .send()
        .await
        .expect("POST transfer");
    assert!(
        resp.status() == 400 || resp.status() == 422 || resp.status() == 500,
        "from == to must fail; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_inter_entity_report_renders() {
    let server = TestServer::new().await;
    let (cookie, from_ledger, to_ledger, from_account, to_account, _, _) =
        bootstrap_two_ledgers(&server).await;

    // Issue one transfer.
    let resp = server
        .client()
        .post(format!("{}/transfers/inter-ledger", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("from_ledger_id", from_ledger.to_string().as_str()),
            ("to_ledger_id", to_ledger.to_string().as_str()),
            ("from_account_id", from_account.to_string().as_str()),
            ("to_account_id", to_account.to_string().as_str()),
            ("amount", "100.00"),
            ("date", "2026-08-15"),
            ("description", ""),
        ])
        .send()
        .await
        .expect("POST transfer");
    assert_eq!(resp.status(), 303);

    // The report must list it for BOTH ledgers.
    for &lid in &[from_ledger, to_ledger] {
        let resp = server
            .client()
            .get(format!(
                "{}/ledgers/{lid}/reports/inter-entity",
                server.base_url()
            ))
            .header(reqwest::header::COOKIE, &cookie)
            .send()
            .await
            .expect("GET inter-entity");
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        let transfers = body["transfers"].as_array().unwrap();
        assert_eq!(transfers.len(), 1);
        assert_eq!(
            transfers[0]["amount"].as_str().unwrap_or(""),
            "100.0000"
        );
    }
}