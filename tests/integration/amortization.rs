//! HTTP + worker tests for amortization schedules (`a10-amortization`).
//!
//! Covers:
//! - `POST /ledgers/{id}/amortization/new` creates a row.
//! - `GET /ledgers/{id}/reports/amortization` lists rows.
//! - `workers::amortization::run_sweep` posts one balanced txn
//!   per due period.
//! - Running the sweep twice on the same day posts nothing
//!   the second time.
//! - `POST .../skip` advances without posting.
//! - A viewer gets 403 when creating.
//! - `GET /ledgers/{id}/reports/amortization` returns the
//!   progress report.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use chrono::NaiveDate;
use rust_decimal::Decimal;
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

async fn bootstrap(server: &TestServer) -> (Uuid, String, String, String) {
    let owner = register(server, "amort-owner@example.com").await;
    let editor = register(server, "amort-editor@example.com").await;
    let viewer = register(server, "amort-viewer@example.com").await;
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("name", "Amort Co"),
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
    let (euid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("amort-editor@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_members (ledger_id, user_id, role) VALUES ($1, $2, 'editor')")
        .bind(ledger_id)
        .bind(euid)
        .execute(&pool)
        .await
        .unwrap();
    let (vuid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("amort-viewer@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ledger_members (ledger_id, user_id, role) VALUES ($1, $2, 'viewer')")
        .bind(ledger_id)
        .bind(vuid)
        .execute(&pool)
        .await
        .unwrap();
    (ledger_id, owner, editor, viewer)
}

#[tokio::test]
async fn http_create_amortization_schedule() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Patent', '1800', 'ASSET', 'INTANGIBLE_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Software & SaaS'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Annual patent amortization"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &expense.to_string()),
            ("total_amount", "1200"),
            ("period_unit", "monthly"),
            ("periods", "12"),
            ("start_date", "2026-01-31"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "create must redirect");

    let row: (Uuid, Decimal, i32, String) = sqlx::query_as(
        "SELECT id, total_amount, periods, period_unit FROM amortization_schedules WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.1, Decimal::new(1200, 0));
    assert_eq!(row.2, 12);
    assert_eq!(row.3, "monthly");
    let _ = (asset, expense);
}

#[tokio::test]
async fn amortization_worker_posts_one_period() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Patent', '1800', 'ASSET', 'INTANGIBLE_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Software & SaaS'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Annual amortization"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &expense.to_string()),
            ("total_amount", "1200"),
            ("period_unit", "monthly"),
            ("periods", "12"),
            ("start_date", "2026-01-15"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // Force next_post_date to today so the sweep picks it up.
    sqlx::query("UPDATE amortization_schedules SET next_post_date = CURRENT_DATE")
        .execute(&pool)
        .await
        .unwrap();

    let actor: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let posted = openaccounting::workers::amortization::run_sweep(
        &pool,
        sqlx::query_scalar::<_, NaiveDate>("SELECT CURRENT_DATE")
            .fetch_one(&pool)
            .await
            .unwrap(),
        actor,
    )
    .await
    .unwrap();
    assert_eq!(posted, 1);

    let txn_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM transactions WHERE ledger_id = $1 AND kind = 'amortization'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(txn_count, 1);
}

#[tokio::test]
async fn amortization_worker_idempotent() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Patent', '1800', 'ASSET', 'INTANGIBLE_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Software & SaaS'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Idempotent"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &expense.to_string()),
            ("total_amount", "600"),
            ("period_unit", "monthly"),
            ("periods", "6"),
            ("start_date", "2026-01-15"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    sqlx::query("UPDATE amortization_schedules SET next_post_date = CURRENT_DATE")
        .execute(&pool)
        .await
        .unwrap();
    let actor: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let first = openaccounting::workers::amortization::run_sweep(
        &pool,
        sqlx::query_scalar::<_, NaiveDate>("SELECT CURRENT_DATE")
            .fetch_one(&pool)
            .await
            .unwrap(),
        actor,
    )
    .await
    .unwrap();
    assert_eq!(first, 1);

    // Don't rewind `next_post_date`; the worker advances it after
    // posting, so the schedule is no longer due. A second sweep
    // must post nothing because nothing is due.
    let second = openaccounting::workers::amortization::run_sweep(
        &pool,
        sqlx::query_scalar::<_, NaiveDate>("SELECT CURRENT_DATE")
            .fetch_one(&pool)
            .await
            .unwrap(),
        actor,
    )
    .await
    .unwrap();
    assert_eq!(second, 0, "second sweep must post nothing");

    let txn_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM transactions WHERE ledger_id = $1 AND kind = 'amortization'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(txn_count, 1, "exactly one amortization transaction");
}

#[tokio::test]
async fn http_amortization_progress_report() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Patent', '1800', 'ASSET', 'INTANGIBLE_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Software & SaaS'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Report Test"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &expense.to_string()),
            ("total_amount", "2400"),
            ("period_unit", "monthly"),
            ("periods", "24"),
            ("start_date", "2026-01-15"),
        ])
        .send()
        .await
        .unwrap();

    let resp = server
        .client()
        .get(format!(
            "{}/ledgers/{ledger_id}/reports/amortization",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let html = resp.text().await.unwrap();
    assert!(html.contains("Report Test"));
    assert!(html.contains("2400"));
    assert!(html.contains("24"));
}

#[tokio::test]
async fn http_amortization_skip_period() {
    let server = TestServer::new().await;
    let (ledger_id, owner, _editor, _viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Patent', '1800', 'ASSET', 'INTANGIBLE_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Software & SaaS'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Skip Test"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &expense.to_string()),
            ("total_amount", "1200"),
            ("period_unit", "monthly"),
            ("periods", "12"),
            ("start_date", "2026-01-15"),
        ])
        .send()
        .await
        .unwrap();
    let (sid,): (Uuid,) =
        sqlx::query_as("SELECT id FROM amortization_schedules WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/{sid}/skip",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("placeholder", "x")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    let skipped: i32 =
        sqlx::query_scalar("SELECT skipped_periods FROM amortization_schedules WHERE id = $1")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(skipped, 1);
}

#[tokio::test]
async fn http_amortization_viewer_cannot_create() {
    let server = TestServer::new().await;
    let (ledger_id, _owner, _editor, viewer) = bootstrap(&server).await;
    let pool = server.db().pool();
    let asset: Uuid = sqlx::query_scalar("SELECT id FROM accounts WHERE ledger_id = $1 LIMIT 1")
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/amortization/new",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &viewer)
        .form(&[
            ("description", "Viewer Test"),
            ("source_account_id", &asset.to_string()),
            ("target_account_id", &asset.to_string()),
            ("total_amount", "100"),
            ("period_unit", "monthly"),
            ("periods", "1"),
            ("start_date", "2026-01-15"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "viewer must not create schedules");
}
