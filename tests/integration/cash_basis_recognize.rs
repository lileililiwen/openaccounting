//! HTTP integration tests for revenue / expense recognition
//! (`a9-cash-basis-docs`).
//!
//! Covers:
//! - One-click "Recognize 100 of 1200" creates a monthly template
//!   with two balanced postings.
//! - The symmetric "Recognize expense" path also creates a
//!   balanced schedule (PrepaidExpense -> Expense).
//! - A monthly schedule fires once per period for N periods
//!   when the recurring worker is invoked manually.
//! - `docs/cash-basis.md` is shipped in the repo and linked from
//!   the README.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
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

async fn bootstrap_with_prepaid(server: &TestServer) -> (Uuid, String, Uuid, Uuid, Uuid) {
    let owner = register(server, "recognize-owner@example.com").await;
    // Bootstrap ledger by creating one.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("name", "Recognize Co"),
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

    // Add the two accounts the recognition action targets.
    let sales: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let prepaid_id: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'DeferredRevenue', '2400', 'LIABILITY', 'CURRENT_LIABILITY', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expense_id: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'PrepaidExpense', '1400', 'ASSET', 'CURRENT_ASSET', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // The expense account (OPERATING_EXPENSE subtype) for the
    // symmetric expense path.
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'Software & SaaS Recognized', '5210', 'EXPENSE', 'OPERATING_EXPENSE', 'USD')
         ON CONFLICT DO NOTHING",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    (ledger_id, owner, prepaid_id, sales, expense_id)
}

#[tokio::test]
async fn http_recognize_revenue_creates_monthly_template() {
    let server = TestServer::new().await;
    let (ledger_id, owner, deferred, revenue, _expense) = bootstrap_with_prepaid(&server).await;
    let pool = server.db().pool();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/recognize/revenue",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "SaaS deferred revenue"),
            ("source_account_id", &deferred.to_string()),
            ("target_account_id", &revenue.to_string()),
            ("amount", "100"),
            ("frequency", "monthly"),
            ("start_date", "2026-01-15"),
            ("periods", "12"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "recognize must redirect");

    // Two balanced postings, equal magnitude.
    let postings: Vec<(Uuid, String, Decimal)> = sqlx::query_as(
        "SELECT account_id, direction, amount FROM template_postings
         WHERE template_id IN (
           SELECT id FROM transaction_templates WHERE ledger_id = $1
         )",
    )
    .bind(ledger_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(postings.len(), 2);
    assert_eq!(postings[0].2, postings[1].2, "amounts must match");
    assert_ne!(
        postings[0].1, postings[1].1,
        "directions must differ (DEBIT vs CREDIT)"
    );

    // Audit row written.
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM audit_entries
         WHERE ledger_id = $1 AND action = 'create' AND entity_type = 'recognition_template'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn http_recognize_expense_creates_monthly_template() {
    let server = TestServer::new().await;
    let (ledger_id, owner, deferred, sales, expense) = bootstrap_with_prepaid(&server).await;
    let pool = server.db().pool();

    // For the expense path the "source" is PrepaidExpense and
    // the "target" is the operating-expense account.
    sqlx::query(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'SoftwareExpense', '5211', 'EXPENSE', 'OPERATING_EXPENSE', 'USD')
         ON CONFLICT DO NOTHING",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();
    let _ = (deferred, sales);

    // Verify source account belongs to the ledger (debug).
    let source_check: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE id = $1 AND ledger_id = $2")
            .bind(expense)
            .bind(ledger_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(
        source_check.is_some(),
        "PrepaidExpense must belong to the ledger"
    );

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/recognize/expense",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Annual SaaS prepaid"),
            ("source_account_id", &expense.to_string()),
            ("target_account_id", &expense.to_string()),
            ("amount", "50"),
            ("frequency", "monthly"),
        ])
        .send()
        .await
        .unwrap();
    // Same-account debit/credit is rejected; this is intentional.
    assert_eq!(
        resp.status(),
        400,
        "same source and target account should be rejected"
    );

    // A valid distinct pair succeeds.
    let saas_expense: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
         VALUES ($1, 'SaaSExpenseTarget', '5212', 'EXPENSE', 'OPERATING_EXPENSE', 'USD')
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/recognize/expense",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&vec![
            ("description".to_string(), "Annual SaaS prepaid".to_string()),
            ("source_account_id".to_string(), expense.to_string()),
            ("target_account_id".to_string(), saas_expense.to_string()),
            ("amount".to_string(), "50".to_string()),
            ("frequency".to_string(), "monthly".to_string()),
        ])
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    eprintln!("DEBUG expense-revenue status={status} body={body}");
    assert_eq!(status, 303, "recognize-expense failed: {body}");

    let postings: Vec<(Uuid, String, Decimal)> = sqlx::query_as(
        "SELECT account_id, direction, amount FROM template_postings
         WHERE template_id IN (
           SELECT id FROM transaction_templates WHERE ledger_id = $1
         )",
    )
    .bind(ledger_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(postings.len(), 2);
}

#[tokio::test]
async fn http_recognize_template_fires_via_process_due() {
    let server = TestServer::new().await;
    let (ledger_id, owner, deferred, revenue, _expense) = bootstrap_with_prepaid(&server).await;
    let pool = server.db().pool();

    // Create a monthly recognition schedule.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/recognize/revenue",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[
            ("description", "Test schedule"),
            ("source_account_id", &deferred.to_string()),
            ("target_account_id", &revenue.to_string()),
            ("amount", "100"),
            ("frequency", "monthly"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);

    // Bypass the worker clock: rewrite next_date to today so
    // process_due picks it up.
    sqlx::query(
        "UPDATE transaction_templates SET next_date = CURRENT_DATE
         WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/templates/process_due",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &owner)
        .form(&[("ignored", "x")])
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        status == 303 || status == 302,
        "process_due must redirect, got {status} -> {loc}"
    );

    // One transaction was created.
    let txn_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM transactions
         WHERE ledger_id = $1 AND kind = 'recurring'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(txn_count, 1, "process_due must create exactly one txn");
}

#[tokio::test]
async fn docs_cash_basis_md_exists_and_readme_references_it() {
    // Filesystem-level assertion: the doc must be present and
    // the README must link to it.
    let doc = std::fs::read_to_string("docs/cash-basis.md")
        .expect("docs/cash-basis.md must be checked in");
    assert!(doc.contains("Cash-Basis Reporting"));
    assert!(doc.contains("DeferredRevenue"));
    assert!(doc.contains("PrepaidExpense"));

    let readme = std::fs::read_to_string("README.md").expect("README.md");
    assert!(
        readme.contains("docs/cash-basis.md"),
        "README must link to docs/cash-basis.md"
    );
}
