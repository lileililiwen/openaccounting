//! HTTP integration tests for editor / viewer role enforcement
//! (`s9-editor-role-enforcement`).
//!
//! Verifies that:
//! - The owner of a ledger can perform every write action (303).
//! - An editor of a ledger can perform every write action
//!   EXCEPT sharing (303 on writes, 403 on `share/invite`).
//! - A viewer of a ledger is rejected on every write (403).
//!
//! The matrix covers transactions, accounts, invoices, budgets, and
//! contacts (task 1.1–1.6 in the change).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

/// Register and log in `email` using a fresh reqwest client (its
/// own cookie jar), so concurrent users don't trample each
/// other's session. Returns the captured `oa_session=<value>`
/// cookie value.
async fn register_and_login(server: &TestServer, email: &str) -> String {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            // CSRF bypass: these tests are not exercising CSRF.
            h.insert(
                "X-OA-CSRF-Bypass",
                reqwest::header::HeaderValue::from_static("1"),
            );
            h
        })
        .build()
        .expect("build per-user client");
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

/// Set up one ledger with one editor and one viewer, returning
/// `(ledger_id, owner_cookie, editor_cookie, viewer_cookie)`.
async fn bootstrap_with_roles(server: &TestServer) -> (Uuid, String, String, String) {
    let owner_email = "owner@example.com";
    let editor_email = "editor@example.com";
    let viewer_email = "viewer@example.com";

    let owner_cookie = register_and_login(server, owner_email).await;
    let editor_cookie = register_and_login(server, editor_email).await;
    let viewer_cookie = register_and_login(server, viewer_email).await;

    let pool = server.db().pool();
    let (editor_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(editor_email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (viewer_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(viewer_email)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Create the ledger via the HTTP endpoint so the default
    // chart of accounts (cash, sales, etc.) is seeded by the
    // handler. The owner cookie is the session we just logged in.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner_cookie)
        .form(&[
            ("name", "Shared"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    assert_eq!(
        resp.status(),
        303,
        "ledger create must succeed; got {}",
        resp.status()
    );
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    for (user_id, role) in [(editor_id, "editor"), (viewer_id, "viewer")] {
        sqlx::query(
            "INSERT INTO ledger_members (ledger_id, user_id, role)
             VALUES ($1, $2, $3)",
        )
        .bind(ledger_id)
        .bind(user_id)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    }

    (ledger_id, owner_cookie, editor_cookie, viewer_cookie)
}

/// Look up an existing account on the ledger (seeded by
/// `ledgers::create`). Returns `(cash_id, sales_id)`.
async fn seed_account_ids(server: &TestServer, ledger_id: Uuid) -> (Uuid, Uuid) {
    let pool = server.db().pool();
    let cash: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (cash, sales)
}

/// POST a balanced two-posting transaction. Returns the response.
async fn post_transaction(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    cash: Uuid,
    sales: Uuid,
) -> reqwest::Response {
    let cash_s = cash.to_string();
    let sales_s = sales.to_string();
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[
            ("date", "2026-08-15"),
            ("description", "Test"),
            ("payee", ""),
            ("reference", ""),
            ("lines[0][account_id]", cash_s.as_str()),
            ("lines[0][direction]", "DEBIT"),
            ("lines[0][amount]", "100.00"),
            ("lines[0][memo]", ""),
            ("lines[1][account_id]", sales_s.as_str()),
            ("lines[1][direction]", "CREDIT"),
            ("lines[1][amount]", "100.00"),
            ("lines[1][memo]", ""),
        ])
        .send()
        .await
        .expect("POST transaction")
}

// ─── transactions (tasks 1.1, 1.2, 1.3) ────────────────────────────────────

#[tokio::test]
async fn http_owner_can_create_transaction() {
    let server = TestServer::new().await;
    let (ledger_id, owner_cookie, _editor, _viewer) = bootstrap_with_roles(&server).await;
    let (cash, sales) = seed_account_ids(&server, ledger_id).await;
    let resp = post_transaction(&server, &owner_cookie, ledger_id, cash, sales).await;
    assert_eq!(
        resp.status(),
        303,
        "owner must be able to create a transaction"
    );
}

#[tokio::test]
async fn http_editor_can_create_transaction() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let (cash, sales) = seed_account_ids(&server, ledger_id).await;
    let resp = post_transaction(&server, &editor_cookie, ledger_id, cash, sales).await;
    assert_eq!(
        resp.status(),
        303,
        "editor must be able to create a transaction (s9)"
    );
}

#[tokio::test]
async fn http_viewer_cannot_create_transaction() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let (cash, sales) = seed_account_ids(&server, ledger_id).await;
    let resp = post_transaction(&server, &viewer_cookie, ledger_id, cash, sales).await;
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from creating a transaction"
    );
}

// ─── accounts, invoices, budgets, contacts (task 1.4) ──────────────────────

#[tokio::test]
async fn http_editor_can_create_account() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/accounts/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .form(&[
            ("name", "Editor Account"),
            ("code", "1001"),
            ("account_type", "ASSET"),
            ("account_subtype", "CURRENT_ASSET"),
            ("description", ""),
        ])
        .send()
        .await
        .expect("POST account as editor");
    assert_eq!(
        resp.status(),
        303,
        "editor must be able to create an account"
    );
}

#[tokio::test]
async fn http_viewer_cannot_create_account() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/accounts/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .form(&[
            ("name", "Viewer Account"),
            ("code", "1002"),
            ("account_type", "ASSET"),
            ("account_subtype", "CURRENT_ASSET"),
            ("description", ""),
        ])
        .send()
        .await
        .expect("POST account as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from creating an account"
    );
}

#[tokio::test]
async fn http_editor_can_create_contact() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/contacts/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .form(&[
            ("name", "Editor Co"),
            ("email", "editor-co@example.com"),
            ("phone", ""),
            ("kind", "customer"),
        ])
        .send()
        .await
        .expect("POST contact as editor");
    assert_eq!(
        resp.status(),
        303,
        "editor must be able to create a contact"
    );
}

#[tokio::test]
async fn http_viewer_cannot_create_contact() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/contacts/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .form(&[
            ("name", "Viewer Co"),
            ("email", "viewer-co@example.com"),
            ("phone", ""),
            ("kind", "customer"),
        ])
        .send()
        .await
        .expect("POST contact as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from creating a contact"
    );
}

#[tokio::test]
async fn http_editor_can_create_invoice() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    // Editor needs at least one contact to file the invoice against.
    let contact_id: Uuid = sqlx::query_scalar(
        "INSERT INTO contacts (ledger_id, name, kind)
         VALUES ($1, 'Editor Contact', 'customer')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let contact_id_s = contact_id.to_string();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/invoices/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .form(&[
            ("contact_id", contact_id_s.as_str()),
            ("kind", "receivable"),
            ("invoice_number", "INV-1"),
            ("invoice_date", "2026-08-15"),
            ("due_date", "2026-09-15"),
            ("total", "100.00"),
        ])
        .send()
        .await
        .expect("POST invoice as editor");
    assert_eq!(
        resp.status(),
        303,
        "editor must be able to create an invoice"
    );
}

#[tokio::test]
async fn http_viewer_cannot_create_invoice() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    let contact_id: Uuid = sqlx::query_scalar(
        "INSERT INTO contacts (ledger_id, name, kind)
         VALUES ($1, 'Viewer Contact', 'customer')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let contact_id_s = contact_id.to_string();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/invoices/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .form(&[
            ("contact_id", contact_id_s.as_str()),
            ("kind", "receivable"),
            ("invoice_number", "INV-2"),
            ("invoice_date", "2026-08-15"),
            ("due_date", "2026-09-15"),
            ("total", "100.00"),
        ])
        .send()
        .await
        .expect("POST invoice as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from creating an invoice"
    );
}

#[tokio::test]
async fn http_editor_can_create_budget() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    let acct: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let acct_s = acct.to_string();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/budgets/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .form(&[
            ("account_id", acct_s.as_str()),
            ("period", "monthly"),
            ("amount", "1000.00"),
            ("alert_threshold", "0.8"),
            ("start_date", "2026-01-01"),
            ("end_date", "2026-12-31"),
        ])
        .send()
        .await
        .expect("POST budget as editor");
    assert_eq!(resp.status(), 303, "editor must be able to create a budget");
}

#[tokio::test]
async fn http_viewer_cannot_create_budget() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    let acct: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let acct_s = acct.to_string();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/budgets/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .form(&[
            ("account_id", acct_s.as_str()),
            ("period", "monthly"),
            ("amount", "1000.00"),
            ("alert_threshold", "0.8"),
            ("start_date", "2026-01-01"),
            ("end_date", "2026-12-31"),
        ])
        .send()
        .await
        .expect("POST budget as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from creating a budget"
    );
}

// ─── sharing (task 1.5) ────────────────────────────────────────────────────

#[tokio::test]
async fn http_editor_cannot_invite_member() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/share/invite",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .form(&[("email", "newperson@example.com"), ("role", "viewer")])
        .send()
        .await
        .expect("POST invite as editor");
    assert_eq!(
        resp.status(),
        403,
        "editor must be forbidden from inviting a new member"
    );
}

#[tokio::test]
async fn http_owner_can_invite_member() {
    let server = TestServer::new().await;
    let (ledger_id, owner_cookie, _editor, _viewer) = bootstrap_with_roles(&server).await;
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/share/invite",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, owner_cookie)
        .form(&[("email", "newperson@example.com"), ("role", "viewer")])
        .send()
        .await
        .expect("POST invite as owner");
    assert_eq!(resp.status(), 303, "owner must be able to invite a member");
}

// ─── viewer cannot delete a resource (task 1.6) ───────────────────────────
//
// There is no `accounts::delete` handler in the codebase; the
// spec's "viewer cannot delete account" is exercised here against
// the budget delete endpoint (`POST /ledgers/{id}/budgets/{bid}/delete`),
// which is the only delete-on-ledger-resource endpoint wired into
// the router. The access pattern (writer check via
// `ensure_writer`) is the same.

#[tokio::test]
async fn http_viewer_cannot_delete_budget() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer_cookie) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    let acct: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // Seed a throwaway budget so the delete has a target id.
    let budget_id: Uuid = sqlx::query_scalar(
        "INSERT INTO budgets (ledger_id, account_id, period, amount, start_date, end_date)
         VALUES ($1, $2, 'monthly', 1000, '2026-01-01', '2026-12-31')
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(acct)
    .fetch_one(&pool)
    .await
    .unwrap();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/budgets/{budget_id}/delete",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, viewer_cookie)
        .send()
        .await
        .expect("POST budget delete as viewer");
    assert_eq!(
        resp.status(),
        403,
        "viewer must be forbidden from deleting a budget"
    );
    // The budget must still exist.
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM budgets WHERE id = $1")
        .bind(budget_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "viewer must NOT have deleted the budget");
}

#[tokio::test]
async fn http_editor_can_delete_budget() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, editor_cookie, _viewer) = bootstrap_with_roles(&server).await;
    let pool = server.db().pool();
    let acct: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let budget_id: Uuid = sqlx::query_scalar(
        "INSERT INTO budgets (ledger_id, account_id, period, amount, start_date, end_date)
         VALUES ($1, $2, 'monthly', 1000, '2026-01-01', '2026-12-31')
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(acct)
    .fetch_one(&pool)
    .await
    .unwrap();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/budgets/{budget_id}/delete",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, editor_cookie)
        .send()
        .await
        .expect("POST budget delete as editor");
    assert_eq!(resp.status(), 303, "editor must be able to delete a budget");
}
