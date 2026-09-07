//! HTTP integration tests for reports polish (`a17-reports-polish`):
//! discoverability of every report and period-close awareness.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
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
        .unwrap()
}

async fn setup(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid, Uuid) {
    let client = make_client();
    let email = format!("{tag}@example.com");
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("username", tag),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    client
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .expect("login");

    let resp = client
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", format!("{tag}-books")),
            ("base_currency", "USD".to_string()),
            ("timezone", "UTC".to_string()),
            ("basis", "accrual".to_string()),
        ])
        .send()
        .await
        .unwrap();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let ledger_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();
    let pool = server.db().pool();
    let (owner,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    (client, ledger_id, owner)
}

#[tokio::test]
async fn index_links_every_rendered_report() {
    let server = TestServer::new().await;
    let (client, ledger_id, _) = setup(&server, "rpidx").await;
    let base = server.base_url();
    let body = client
        .get(format!("{base}/ledgers/{ledger_id}/reports"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    for href in [
        "/reports/trial-balance",
        "/reports/balance-sheet",
        "/reports/income-statement",
        "/reports/cash-flow",
        "/reports/general-ledger",
        "/reports/ar-aging",
        "/reports/ap-aging",
        "/reports/cash-flow-forecast",
        "/budgets/report",
        "/taxes/report",
        "/reports/amortization",
    ] {
        assert!(body.contains(href), "index missing link: {href}");
    }
}

#[tokio::test]
async fn index_shows_closed_periods_when_any() {
    let server = TestServer::new().await;
    let (client, ledger_id, owner) = setup(&server, "rpclosed").await;
    let base = server.base_url();
    let pool = server.db().pool();

    // Fresh: no note.
    let body = client
        .get(format!("{base}/ledgers/{ledger_id}/reports"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !body.contains("Closed periods"),
        "no note expected on fresh ledger"
    );

    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by) VALUES ($1, 2025, $2)",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    let body = client
        .get(format!("{base}/ledgers/{ledger_id}/reports"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("Closed periods: FY2025"));
}

#[tokio::test]
async fn trial_balance_notice_appears_after_close() {
    let server = TestServer::new().await;
    let (client, ledger_id, owner) = setup(&server, "rptb").await;
    let base = server.base_url();
    let pool = server.db().pool();
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by) VALUES ($1, 2025, $2)",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    // As of a date at/after the closed year -> notice.
    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/reports/trial-balance?as_of=2026-01-01"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("FY2025 is closed"),
        "expected closed notice, got: {}",
        body.chars().take(200).collect::<String>()
    );

    // As of before the closed year -> no notice.
    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/reports/trial-balance?as_of=2024-12-31"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !body.contains("is closed"),
        "no notice expected before the closed year"
    );
}

#[tokio::test]
async fn income_statement_notice_intersects_closed_year() {
    let server = TestServer::new().await;
    let (client, ledger_id, owner) = setup(&server, "rpis").await;
    let base = server.base_url();
    let pool = server.db().pool();
    sqlx::query(
        "INSERT INTO closed_periods (ledger_id, period_year, closed_by) VALUES ($1, 2025, $2)",
    )
    .bind(ledger_id)
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();

    // Period intersects 2025 -> notice.
    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/reports/income-statement?from=2025-01-01&to=2025-12-31"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("FY2025 is closed"),
        "expected closed notice, got: {}",
        body.chars().take(200).collect::<String>()
    );

    // Period fully after 2025 -> no notice.
    let body = client
        .get(format!(
            "{base}/ledgers/{ledger_id}/reports/income-statement?from=2026-01-01&to=2026-12-31"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !body.contains("is closed"),
        "no notice expected outside the closed year"
    );
}
