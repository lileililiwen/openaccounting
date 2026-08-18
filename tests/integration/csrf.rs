//! CSRF protection integration tests (`s1-csrf-protection`).
//!
//! `TestServer::client()` adds `X-OA-CSRF-Bypass: 1` as a default
//! header so the rest of the suite can POST urlencoded forms
//! without threading the token through every test. The CSRF tests
//! build their own `reqwest::Client` without that header so they
//! exercise the real verification path.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::common::*;
use reqwest::header::HeaderMap;
use uuid::Uuid;

const USERNAME: &str = "csrf_alice";
const EMAIL: &str = "csrf_alice@example.com";
const PASSWORD: &str = "correct horse battery staple";

async fn fresh_strict_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(HeaderMap::new())
        .build()
        .expect("build strict client")
}

async fn login_strict(client: &reqwest::Client, base_url: &str) {
    client
        .post(format!("{base_url}/register"))
        .form(&[
            ("email", EMAIL),
            ("username", USERNAME),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    client
        .post(format!("{base_url}/login"))
        .form(&[
            ("email", EMAIL),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .expect("login");
}

#[tokio::test]
async fn http_csrf_token_present_on_ledger_dashboard() {
    let server = TestServer::new().await;
    let bootstrap = server
        .bootstrap_user("csrf_bootstrap", "csrf_bootstrap@example.com", PASSWORD)
        .await;
    // Create a ledger so the dashboard page exists.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, bootstrap)
        .form(&[
            ("name", "CSRF"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("create ledger");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let strict = fresh_strict_client().await;
    login_strict(&strict, server.base_url()).await;
    // Strict client needs to be the owner of the ledger to view it;
    // we just registered the strict user. Make the strict user the
    // owner by updating the ledger's owner_id directly.
    let pool = server.db().pool();
    let (strict_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ledgers SET owner_id = $1 WHERE id = $2")
        .bind(strict_id)
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    let resp = strict
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .send()
        .await
        .expect("dashboard");
    let body = resp.text().await.unwrap();
    let token = crate::common::extract_csrf_token(&body)
        .expect("dashboard must carry csrf_token via meta or hidden input");
    assert!(!token.is_empty());
}

#[tokio::test]
async fn http_post_without_token_rejected() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("csrf_reject", "csrf_reject@example.com", PASSWORD)
        .await;
    // Create a ledger via the bypass client.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Reject Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let strict = fresh_strict_client().await;
    login_strict(&strict, server.base_url()).await;
    // Make strict user the owner.
    let pool = server.db().pool();
    let (strict_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ledgers SET owner_id = $1 WHERE id = $2")
        .bind(strict_id)
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    // POST a transaction without any csrf token → 403, no rows.
    let resp = strict
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .form(&[
            ("date", "2026-08-01"),
            ("description", "Coffee"),
            ("payee", "Mogador"),
        ])
        .send()
        .await
        .expect("POST without csrf");
    assert_eq!(resp.status(), 403, "missing csrf must yield 403");
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "no rows must be written on 403");
}

#[tokio::test]
async fn http_post_with_valid_token_succeeds() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("csrf_ok", "csrf_ok@example.com", PASSWORD)
        .await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "OK Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let strict = fresh_strict_client().await;
    login_strict(&strict, server.base_url()).await;
    let pool = server.db().pool();
    let (strict_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ledgers SET owner_id = $1 WHERE id = $2")
        .bind(strict_id)
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    // Fetch the dashboard to obtain the csrf token.
    let dashboard = strict
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .send()
        .await
        .expect("dashboard");
    let body = dashboard.text().await.unwrap();
    let token = crate::common::extract_csrf_token(&body).expect("dashboard csrf token");

    // Find the Cash on Hand and Sales Revenue account ids.
    let (cash_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (sales_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let body_str = format!(
        "csrf_token={token}&date=2026-08-01&description=Coffee&payee=Mogador&lines[0][account_id]={cash_id}&lines[0][amount]=42.50&lines[0][direction]=DEBIT&lines[1][account_id]={sales_id}&lines[1][amount]=42.50&lines[1][direction]=CREDIT"
    );
    let resp = strict
        .post(format!(
            "{}/ledgers/{ledger_id}/transactions/new",
            server.base_url()
        ))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body_str)
        .send()
        .await
        .expect("POST with csrf");
    assert_eq!(
        resp.status(),
        303,
        "valid csrf must yield 303; got {}",
        resp.status()
    );
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn http_token_invalidated_on_logout() {
    let server = TestServer::new().await;
    let strict = fresh_strict_client().await;
    login_strict(&strict, server.base_url()).await;
    let pool = server.db().pool();
    let (strict_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, 'T', 'USD', 'UTC', 'accrual')",
    )
    .bind(strict_id)
    .execute(&pool)
    .await
    .unwrap();
    let (ledger_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM ledgers WHERE owner_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(strict_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Issue a csrf token from a dashboard GET.
    let body = strict
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let token = crate::common::extract_csrf_token(&body).expect("token");

    // Logout (POST /logout requires csrf when authenticated).
    let resp = strict
        .post(format!("{}/logout", server.base_url()))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(format!("csrf_token={token}"))
        .send()
        .await
        .expect("logout");
    assert!(
        resp.status().is_redirection(),
        "logout must redirect; got {}",
        resp.status()
    );

    // Re-login (creates a fresh session + fresh csrf token).
    login_strict(&strict, server.base_url()).await;
    let (new_id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM ledgers WHERE owner_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(strict_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE ledgers SET owner_id = $1 WHERE id = $2")
        .bind(strict_id)
        .bind(new_id)
        .execute(&pool)
        .await
        .unwrap();

    // Replaying the pre-logout token on a fresh session must
    // be rejected: the new session's csrf differs.
    let resp = strict
        .post(format!(
            "{}/ledgers/{new_id}/transactions/new",
            server.base_url()
        ))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(format!(
            "csrf_token={token}&date=2026-08-01&description=Coffee"
        ))
        .send()
        .await
        .expect("post after re-login");
    assert_eq!(
        resp.status(),
        403,
        "replayed token after logout-then-login must be rejected; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn http_webhook_plaid_is_exempt() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("csrf_plaid", "csrf_plaid@example.com", PASSWORD)
        .await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "Plaid Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    // PLAID_WEBHOOK_SECRET is unset in tests, so the handler
    // accepts any payload. The endpoint is on the public router
    // so CSRF middleware never runs.
    let strict = fresh_strict_client().await;
    let resp = strict
        .post(format!(
            "{}/ledgers/{ledger_id}/webhooks/plaid",
            server.base_url()
        ))
        .json(&serde_json::json!({ "webhook_type": "ITEM", "item_id": "x" }))
        .send()
        .await
        .expect("plaid webhook");
    assert_eq!(resp.status(), 200, "plaid webhook must succeed");
}

#[tokio::test]
async fn http_htmx_meta_token_present_in_head() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("csrf_htmx", "csrf_htmx@example.com", PASSWORD)
        .await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &cookie)
        .form(&[
            ("name", "HTMX Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    let ledger_id = Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();

    let strict = fresh_strict_client().await;
    login_strict(&strict, server.base_url()).await;
    let pool = server.db().pool();
    let (strict_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(EMAIL)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ledgers SET owner_id = $1 WHERE id = $2")
        .bind(strict_id)
        .bind(ledger_id)
        .execute(&pool)
        .await
        .unwrap();

    let body = strict
        .get(format!(
            "{}/ledgers/{ledger_id}/dashboard",
            server.base_url()
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    // The post-processing middleware injects the token both as
    // a `<meta>` tag (so HTMX can read it) and as a hidden input
    // (so non-JS forms include it).
    assert!(
        body.contains(r#"<meta name="csrf-token" content=""#),
        "head must carry the csrf-token meta tag; body did not contain it"
    );
}
