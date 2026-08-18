//! HTTP integration tests for transaction edit / reversal
//! (`a2-transaction-edit-void`).
//!
//! Covers:
//! - `POST /transactions/{id}/reverse` creates a `reversing` row
//!   that nets the original to zero.
//! - `POST /transactions/{id}/edit` creates a `reversing` row
//!   PLUS the corrected transaction in one atomic tx.
//! - A viewer gets 403 on `reverse`.
//! - Reversing a `reversing` row returns 422.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

/// Register and log in `email` with a fresh reqwest client
/// (independent cookie jar).
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

/// Bootstrap: create a ledger, return cookies for owner + editor + viewer.
async fn bootstrap(server: &TestServer) -> (Uuid, String, String, String) {
    let owner = register_and_login(server, "txn-owner@example.com").await;
    let editor = register_and_login(server, "txn-editor@example.com").await;
    let viewer = register_and_login(server, "txn-viewer@example.com").await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("name", "Edit Co"),
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
    for (email, role) in [
        ("txn-editor@example.com", "editor"),
        ("txn-viewer@example.com", "viewer"),
    ] {
        let (uid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO ledger_members (ledger_id, user_id, role)
             VALUES ($1, $2, $3)",
        )
        .bind(ledger_id)
        .bind(uid)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    }
    (ledger_id, owner, editor, viewer)
}

async fn create_test_txn(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    description: &str,
) -> Uuid {
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
    let txn_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, created_by)
         VALUES ($1, '2026-08-15', $2, 'USD', (SELECT id FROM users LIMIT 1))
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(description)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO postings (transaction_id, account_id, amount, direction)
         VALUES ($1, $2, 100, 'DEBIT'),
                ($1, $3, 100, 'CREDIT')",
    )
    .bind(txn_id)
    .bind(cash)
    .bind(sales)
    .execute(&pool)
    .await
    .unwrap();
    // Mark the user-id cookie creator as the txn author (best
    // effort — the test just cares about the route behaviour).
    let _ = cookie;
    txn_id
}

#[tokio::test]
async fn http_reverse_transaction_creates_pair() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let txn_id = create_test_txn(&server, &owner, ledger_id, "Reversible").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("memo", "void it")])
        .send()
        .await
        .expect("POST reverse");
    assert_eq!(resp.status(), 303, "reverse must redirect (303)");

    // Verify the reversal row exists.
    let pool = server.db().pool();
    let reversal: (Uuid, String, Uuid) = sqlx::query_as(
        "SELECT id, kind, reverses_id FROM transactions
         WHERE reverses_id = $1 LIMIT 1",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reversal.1, "reversing", "kind must be `reversing`");
    assert_eq!(reversal.2, txn_id);

    // Original unchanged.
    let original_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE id = $1 AND kind = 'standard'")
            .bind(txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(original_count.0, 1);

    // The reversal postings sum to the negative of the originals.
    let net: (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN  p.amount ELSE -p.amount END), 0)::DECIMAL
         FROM postings p
         WHERE p.transaction_id IN ($1, $2)",
    )
    .bind(txn_id)
    .bind(reversal.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(net.0, Decimal::ZERO, "original + reversal must net to zero");
}

#[tokio::test]
async fn http_edit_transaction_creates_pair() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let txn_id = create_test_txn(&server, &owner, ledger_id, "Edit Me").await;

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

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/edit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "Corrected"),
            ("payee", ""),
            ("reference", ""),
            ("lines[0][account_id]", &cash.to_string()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "200.00"),
            ("lines[0][memo]", ""),
            ("lines[1][account_id]", &sales.to_string()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "200.00"),
            ("lines[1][memo]", ""),
        ])
        .send()
        .await
        .expect("POST edit");
    assert_eq!(resp.status(), 303);

    // A reversal row + a corrected `standard` row.
    let row: (i64, i64) = sqlx::query_as(
        "SELECT
            SUM(CASE WHEN reverses_id = $1 AND kind = 'reversing' THEN 1 ELSE 0 END) AS reversal_count,
            SUM(CASE WHEN kind = 'standard' AND reverses_id IS NULL AND description = 'Corrected' THEN 1 ELSE 0 END) AS corrected_count
         FROM transactions
         WHERE ledger_id = $2",
    )
    .bind(txn_id)
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 1, "exactly one reversal row");
    assert_eq!(row.1, 1, "exactly one corrected row");
}

#[tokio::test]
async fn http_reverse_viewer_403() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, viewer) = bootstrap(&server).await;
    let txn_id = create_test_txn(&server, &owner, ledger_id, "Viewer can't see").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &viewer)
        .form(&[("memo", "")])
        .send()
        .await
        .expect("POST reverse as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from reversing; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_reverse_reversal_422() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let txn_id = create_test_txn(&server, &owner, ledger_id, "Reversible").await;

    // First reverse to create a `reversing` row.
    let r1 = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("memo", "")])
        .send()
        .await
        .expect("POST first reverse");
    assert_eq!(r1.status(), 303);

    let pool = server.db().pool();
    let reversal_id: Uuid =
        sqlx::query_scalar("SELECT id FROM transactions WHERE reverses_id = $1 LIMIT 1")
            .bind(txn_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Now try to reverse the reversal itself.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{reversal_id}/reverse",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("memo", "")])
        .send()
        .await
        .expect("POST reverse-of-reversal");
    assert_eq!(
        resp.status(),
        422,
        "reversing a reversal must return 422; got {}",
        resp.status()
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.to_lowercase().contains("cannot reverse"),
        "body must explain the rejection; got {body}"
    );
}

#[tokio::test]
async fn db_reversal_link_row_updated() {
    // Unit-ish: insert a txn, reverse it, verify `reverses_id`
    // on the new row points at the original.
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let txn_id = create_test_txn(&server, &owner, ledger_id, "link test").await;

    // Insert the reversal directly to exercise the column.
    let reversal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, currency, kind, created_by, reverses_id)
         VALUES ($1, '2026-08-16', 'r', 'USD', 'reversing', (SELECT id FROM users LIMIT 1), $2)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let row: (Uuid, String, Uuid) =
        sqlx::query_as("SELECT id, kind, reverses_id FROM transactions WHERE id = $1")
            .bind(reversal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, reversal_id);
    assert_eq!(row.1, "reversing");
    assert_eq!(row.2, txn_id, "reverses_id must point at the original");
}
