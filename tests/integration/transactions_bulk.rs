//! HTTP integration tests for bulk transaction actions
//! (`u3-bulk-actions`).
//!
//! Verifies that:
//! - Bulk-tagging three rows inserts three transaction_tags
//!   rows (idempotent — re-running attaches no duplicates).
//! - Bulk-untagging removes the tag from every selected row.
//! - Bulk-contact sets `contact_id` on every selected row.
//! - Bulk-delete creates one reversing transaction per
//!   original and leaves the originals intact.
//! - The 500-row limit rejects with 422.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

/// Create a ledger via the HTTP endpoint so the default chart
/// of accounts is seeded.
async fn make_ledger(server: &TestServer, cookie: &str) -> uuid::Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("name", "Bulk Co"),
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

/// Seed three balanced transactions for the ledger.
async fn seed_three_txns(server: &TestServer, ledger_id: uuid::Uuid) -> Vec<uuid::Uuid> {
    let pool = server.db().pool();
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (cash,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sales,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut ids = Vec::new();
    for i in 0..3 {
        let txn_id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
             VALUES ($1, '2026-08-15', $2, 'USD', $3) RETURNING id",
        )
        .bind(ledger_id)
        .bind(format!("txn-{i}"))
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let amt = rust_decimal::Decimal::new(1000 + i as i64, 0);
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction)
             VALUES ($1, $2, $3, 'DEBIT'),
                    ($1, $4, $3, 'CREDIT')",
        )
        .bind(txn_id)
        .bind(cash)
        .bind(amt)
        .bind(sales)
        .execute(&pool)
        .await
        .unwrap();
        ids.push(txn_id);
    }
    ids
}

/// POST a bulk action with the given action + value.
async fn bulk(
    server: &TestServer,
    cookie: &str,
    ledger_id: uuid::Uuid,
    action: &str,
    value: &str,
    ids: &[uuid::Uuid],
) -> reqwest::Response {
    let body = format!(
        "txn_ids={}&action={}&value={}",
        ids.iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(","),
        urlencoded(action),
        urlencoded(value),
    );
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/bulk",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("bulk POST")
}

fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

#[tokio::test]
async fn http_bulk_tag() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "alice_bulk",
            "alice_bulk@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;
    let ids = seed_three_txns(&server, ledger_id).await;

    let resp = bulk(&server, &cookie, ledger_id, "tag", "personal", &ids).await;
    let status = resp.status();
    assert!(
        status == 303 || status == 302,
        "bulk tag must redirect; got {status}"
    );

    // Three new transaction_tags rows.
    let pool = server.db().pool();
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transaction_tags tt
         JOIN tags t ON t.id = tt.tag_id
         WHERE t.ledger_id = $1 AND t.name = 'personal'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3);

    // Re-running attaches no duplicates.
    let resp = bulk(&server, &cookie, ledger_id, "tag", "personal", &ids).await;
    assert!(resp.status() == 303 || resp.status() == 302);
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transaction_tags tt
         JOIN tags t ON t.id = tt.tag_id
         WHERE t.ledger_id = $1 AND t.name = 'personal'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3, "tag must be idempotent");
}

#[tokio::test]
async fn http_bulk_untag() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "bob_bulk",
            "bob_bulk@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;
    let ids = seed_three_txns(&server, ledger_id).await;

    // Tag first.
    let resp = bulk(&server, &cookie, ledger_id, "tag", "personal", &ids).await;
    assert!(resp.status() == 303 || resp.status() == 302);

    // Then untag.
    let resp = bulk(&server, &cookie, ledger_id, "untag", "personal", &ids).await;
    assert!(resp.status() == 303 || resp.status() == 302);

    let pool = server.db().pool();
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transaction_tags tt
         JOIN tags t ON t.id = tt.tag_id
         WHERE t.ledger_id = $1 AND t.name = 'personal'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 0, "untag must remove every attachment");
}

#[tokio::test]
async fn http_bulk_contact() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user(
            "carol_bulk",
            "carol_bulk@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;
    let ids = seed_three_txns(&server, ledger_id).await;

    // Create a contact.
    let contact_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO contacts (ledger_id, name, kind)
         VALUES ($1, 'Acme', 'customer') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = bulk(
        &server,
        &cookie,
        ledger_id,
        "contact",
        &contact_id.to_string(),
        &ids,
    )
    .await;
    assert!(resp.status() == 303 || resp.status() == 302);

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND contact_id = $2",
    )
    .bind(ledger_id)
    .bind(contact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3, "all 3 rows must gain the contact");
}

#[tokio::test]
async fn http_bulk_delete_uses_reversals() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "dave_bulk",
            "dave_bulk@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;
    let ids = seed_three_txns(&server, ledger_id).await;

    let resp = bulk(&server, &cookie, ledger_id, "delete", "", &ids).await;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status == 303 || status == 302,
        "bulk delete must redirect; got {status} body={body}"
    );

    let pool = server.db().pool();

    // Original transactions preserved.
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind = 'standard'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3, "originals must remain intact");

    // Three new reversing transactions.
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1 AND kind = 'reversing'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3, "three reversal transactions expected");

    // Postings on the reversals must mirror the originals
    // (sum to zero net).
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM postings p
         JOIN transactions t ON t.id = p.transaction_id
         WHERE t.ledger_id = $1 AND t.kind = 'reversing'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 6, "three reversal transactions × two legs each");
}

#[tokio::test]
async fn http_bulk_limit_422() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "eve_bulk",
            "eve_bulk@example.com",
            "correct horse battery staple",
        )
        .await;
    let ledger_id = make_ledger(&server, &cookie).await;

    // 501 fake uuids.
    let fake_ids: Vec<String> = (0..501)
        .map(|_| uuid::Uuid::nil().simple().to_string())
        .collect();
    let body = format!("txn_ids={}&action=delete&value=", fake_ids.join(","));

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/bulk",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("bulk POST");
    let status = resp.status();
    assert_eq!(
        status, 422,
        "501 selected rows must reject with 422; got {status}"
    );
}
