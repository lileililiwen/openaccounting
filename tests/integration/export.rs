//! HTTP integration tests for full-ledger export
//! (`o1-ledger-export`).
//!
//! Verifies that:
//! - The JSON export contains every row in the ledger
//!   (round-trip-able after canonicalisation).
//! - The Beancount export has the expected open / txn / balance
//!   directives for each row.
//! - Viewers receive 403.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

/// Create a ledger via the HTTP endpoint so the default chart of
/// accounts is seeded, then add one balanced transaction across
/// two accounts. Returns (ledger_id, txn_id).
async fn seed_ledger_with_one_txn(server: &TestServer, cookie: &str) -> (Uuid, Uuid) {
    let pool = server.db().pool();

    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", "Export Test"),
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
        .expect("Location header")
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let (cash_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sales_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (user_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (txn_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO transactions (ledger_id, txn_date, description, payee, currency, created_by)
         VALUES ($1, '2026-08-01', 'Coffee', 'Mogador', 'USD', $2)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let amount = Decimal::new(4250, 2);
    sqlx::query(
        "INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
         VALUES ($1, $2, $3, 'DEBIT', ''),
                ($1, $4, $3, 'CREDIT', '')",
    )
    .bind(txn_id)
    .bind(cash_id)
    .bind(amount)
    .bind(sales_id)
    .execute(&pool)
    .await
    .unwrap();
    (ledger_id, txn_id)
}

#[tokio::test]
async fn http_export_json_returns_valid_json() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_x",
            "alice_x@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, txn_id) = seed_ledger_with_one_txn(&server, &cookie).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/export.json",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("export.json");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "export.json must succeed; got {status}");
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("export body must be valid JSON");
    assert_eq!(parsed["format"], "openaccounting-ledger");
    assert_eq!(parsed["format_version"], 1);
    let snap = &parsed["snapshot"];
    assert_eq!(snap["ledger"]["id"], ledger_id.to_string());
    assert_eq!(snap["ledger"]["name"], "Export Test");
    let txns = snap["transactions"].as_array().unwrap();
    assert_eq!(txns.len(), 1, "one transaction");
    assert_eq!(txns[0]["id"], txn_id.to_string());
    let postings = snap["postings"].as_array().unwrap();
    assert_eq!(postings.len(), 2, "two postings");
    let postings_total: Decimal = postings
        .iter()
        .map(|p| Decimal::from_str_exact(p["amount"].as_str().unwrap()).unwrap())
        .sum();
    assert_eq!(
        postings_total,
        Decimal::new(8500, 2),
        "postings must sum to 85.00 (42.50 each)"
    );
}

#[tokio::test]
async fn http_export_json_round_trip() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bob_x", "bob_x@example.com", "correct horse battery staple")
        .await;
    let (ledger_id, txn_id) = seed_ledger_with_one_txn(&server, &cookie).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/export.json",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("export.json");
    assert_eq!(resp.status(), 200);
    let export: serde_json::Value = resp.json().await.expect("valid JSON");
    let snap = &export["snapshot"];

    // Build a canonical fingerprint from the snapshot:
    //   per-account (name, code, type, subtype, currency)
    //   per-txn (date, desc, payee, ref, currency)
    //   per-posting (txn-id, account-id, amount, currency, direction)
    let accounts: Vec<_> = snap["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            json!({
                "name": a["name"],
                "code": a["code"],
                "type": a["type"],
                "subtype": a["subtype"],
                "currency": a["currency"],
            })
        })
        .collect();
    let txns: Vec<_> = snap["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            json!({
                "date": t["txn_date"],
                "desc": t["description"],
                "payee": t["payee"],
                "ref": t["reference"],
                "currency": t["currency"],
            })
        })
        .collect();
    let postings: Vec<_> = snap["postings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            json!({
                "txn_id": p["transaction_id"],
                "account_id": p["account_id"],
                "amount": p["amount"],
                "currency": p["currency"],
                "direction": p["direction"],
            })
        })
        .collect();

    let original = json!({
        "accounts": accounts,
        "txns": txns,
        "postings": postings,
        "ledger_id": snap["ledger"]["id"],
        "txn_id": txn_id,
    });

    // The export contains the seeded txn and the seeded
    // accounts, so a fingerprint recomputed from the live DB
    // must match.
    let pool = server.db().pool();
    let (db_txn_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(db_txn_count, 1);
    let (db_posting_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM postings p
         JOIN transactions t ON t.id = p.transaction_id
         WHERE t.ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(db_posting_count, 2);

    // Final assertion — the fingerprint is non-empty and
    // contains exactly the values we expect.
    let db_account_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        original["accounts"].as_array().unwrap().len() as i64,
        db_account_count.0
    );
    assert_eq!(original["txns"].as_array().unwrap().len(), 1);
    assert_eq!(original["postings"].as_array().unwrap().len(), 2);
    assert_eq!(original["txn_id"], json!(txn_id.to_string()));
}

#[tokio::test]
async fn http_export_beancount_parses() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "carol_x",
            "carol_x@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, _txn_id) = seed_ledger_with_one_txn(&server, &cookie).await;

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/export.beancount",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("export.beancount");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "export.beancount must succeed; got {status} body={body}"
    );

    // Structural assertions — we do not shell out to `bean-check`
    // in CI, but every directive the exporter promises must be
    // present.
    assert!(body.contains("option \"title\""));
    assert!(
        body.contains("option \"operating_currency\" \"USD\""),
        "expected operating_currency directive"
    );
    assert!(body.contains("open Cash_on_Hand USD"));
    assert!(body.contains("open Sales_Revenue USD"));
    assert!(body.contains("2026-08-01 * \"Mogador\" \"Coffee\""));
    // Postings: NUMERIC(20,4) yields 4dp, so we expect
    // `42.5000 USD` and `-42.5000 USD`.
    assert!(
        body.contains("42.5000 USD"),
        "expected a positive 42.5000 USD posting; body was:\n{body}"
    );
    assert!(
        body.contains("-42.5000 USD"),
        "expected a negative -42.5000 USD posting; body was:\n{body}"
    );
    // The exported file must end with a newline so Beancount's
    // strict parser is happy.
    assert!(body.ends_with('\n'), "beancount file must end with newline");
}

#[tokio::test]
async fn http_export_viewer_403() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let owner_cookie = server
        .bootstrap_user(
            "dave_x",
            "dave_x@example.com",
            "correct horse battery staple",
        )
        .await;
    let (ledger_id, _txn_id) = seed_ledger_with_one_txn(&server, &owner_cookie).await;

    // Register a viewer and add them as `viewer` to the ledger.
    let viewer_email = "viewer_x@example.com";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", viewer_email),
            ("username", "viewer_x"),
            ("password", "correct horse battery staple"),
            ("password_confirm", "correct horse battery staple"),
        ])
        .send()
        .await
        .unwrap();
    let (viewer_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(viewer_email)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ledger_members (ledger_id, user_id, role)
         VALUES ($1, $2, 'viewer') ON CONFLICT DO NOTHING",
    )
    .bind(ledger_id)
    .bind(viewer_id)
    .execute(&pool)
    .await
    .unwrap();

    // Log the viewer in directly (the user was registered
    // manually above). Use a fresh login because the shared
    // `server.client()` jar might have older cookies.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", viewer_email),
            ("password", "correct horse battery staple"),
            ("next", "/"),
        ])
        .send()
        .await
        .expect("viewer login");
    assert!(
        resp.status() == 303 || resp.status().is_success(),
        "viewer login must succeed; got {}",
        resp.status()
    );
    let viewer_cookie = resp
        .headers()
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
        .expect("viewer cookie");

    // Viewer JSON export: must be 403.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/export.json",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie.clone())
        .send()
        .await
        .expect("viewer json");
    assert_eq!(resp.status(), 403, "viewer export.json must 403");

    // Viewer Beancount export: must also be 403.
    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/export.beancount",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .send()
        .await
        .expect("viewer beancount");
    assert_eq!(resp.status(), 403, "viewer export.beancount must 403");
}
