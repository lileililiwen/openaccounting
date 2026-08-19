//! HTTP integration tests for the per-ledger append-only toggle
//! (`d2-append-only-mode`).
//!
//! Covers:
//! - `POST /ledgers/{id}/append_only` toggles the flag (owner only).
//! - An editor gets 403 on the toggle.
//! - In append-only mode, `POST /transactions/{id}/edit` returns 422.
//! - In append-only mode, `POST /transactions/{id}/reverse` STILL
//!   succeeds (reversals are allowed).
//! - In append-only mode, the DB trigger blocks raw `UPDATE accounts`
//!   with SQLSTATE P0001.

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

async fn bootstrap(server: &TestServer) -> (Uuid, String, String) {
    let owner = register(server, "ap-owner@example.com").await;
    let editor = register(server, "ap-editor@example.com").await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("name", "AppendOnly Co"),
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
    let (uid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("ap-editor@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ledger_members (ledger_id, user_id, role)
         VALUES ($1, $2, 'editor')",
    )
    .bind(ledger_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    (ledger_id, owner, editor)
}

async fn post_txn(server: &TestServer, cookie: &str, ledger_id: Uuid, desc: &str) -> Uuid {
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
    .bind(desc)
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
    let _ = cookie;
    txn_id
}

#[tokio::test]
async fn http_append_only_toggle_owner_succeeds() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor) = bootstrap(&server).await;
    let pool = server.db().pool();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/append_only",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("enabled", "true"), ("redirect_to", "/ledgers/{id}")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "owner toggle must redirect");

    let flag: (bool,) = sqlx::query_as("SELECT append_only FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(flag.0, "append_only must be true");

    // Toggle off.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/append_only",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("enabled", "false"), ("redirect_to", "/ledgers/{id}")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let flag: (bool,) = sqlx::query_as("SELECT append_only FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!flag.0);
}

#[tokio::test]
async fn http_append_only_toggle_editor_forbidden() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor) = bootstrap(&server).await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/append_only",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &editor)
        .form(&[("enabled", "true"), ("redirect_to", "/ledgers/{id}")])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "editor must not be able to toggle append_only"
    );
}

#[tokio::test]
async fn http_append_only_blocks_edit_returns_422() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor) = bootstrap(&server).await;
    let pool = server.db().pool();
    sqlx::query("UPDATE ledgers SET append_only = TRUE WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();
    let txn_id = post_txn(&server, &owner, ledger_id, "Locked").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/edit",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "Locked (modified)"),
            (
                "lines[0][account_id]",
                &sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
                )
                .bind(ledger_id)
                .fetch_one(&pool)
                .await
                .unwrap()
                .to_string(),
            ),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100"),
            (
                "lines[1][account_id]",
                &sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'",
                )
                .bind(ledger_id)
                .fetch_one(&pool)
                .await
                .unwrap()
                .to_string(),
            ),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "100"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        422,
        "edits must be rejected with 422 in append-only mode"
    );
}

#[tokio::test]
async fn http_append_only_allows_reverse() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor) = bootstrap(&server).await;
    let pool = server.db().pool();
    sqlx::query("UPDATE ledgers SET append_only = TRUE WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();
    let txn_id = post_txn(&server, &owner, ledger_id, "To Reverse").await;

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("memo", "Reversal under append-only")])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        303,
        "reversals MUST be allowed in append-only mode"
    );

    let reversal_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM transactions
         WHERE reverses_id = $1 AND kind = 'reversing'",
    )
    .bind(txn_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reversal_count.0, 1);
}

#[tokio::test]
async fn http_append_only_blocks_raw_account_update() {
    // Bypass the handler; talk directly to Postgres. The DB trigger
    // is the last line of defence.
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor) = bootstrap(&server).await;
    let pool = server.db().pool();
    sqlx::query("UPDATE ledgers SET append_only = TRUE WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();
    let acct: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let result = sqlx::query("UPDATE accounts SET name = 'Renamed' WHERE id = $1")
        .bind(acct)
        .execute(&pool)
        .await;
    let err = result.expect_err("raw UPDATE on accounts must be blocked by trigger");
    let msg = err.to_string();
    assert!(
        msg.contains("append-only") || msg.contains("P0001"),
        "trigger should signal P0001 / append-only, got: {msg}"
    );
}

#[tokio::test]
async fn http_append_only_editor_cannot_disable() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor) = bootstrap(&server).await;
    let pool = server.db().pool();
    sqlx::query("UPDATE ledgers SET append_only = TRUE WHERE id = $1")
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/append_only",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &editor)
        .form(&[("enabled", "false"), ("redirect_to", "/ledgers/{id}")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    let flag: (bool,) = sqlx::query_as("SELECT append_only FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(flag.0, "editor must NOT be able to disable append-only");
}
