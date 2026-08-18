//! HTTP integration tests for the Prometheus metrics endpoint
//! (`o4-metrics-endpoint`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

/// `GET /metrics` must return 200 with the exposition format.
#[tokio::test]
async fn http_metrics_endpoint_returns_200_prometheus_format() {
    let server = TestServer::new().await;
    // Seed the recorder with at least one counter so the scrape
    // is non-empty (the global recorder starts blank per process).
    let _ = server
        .client()
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .expect("GET /healthz to seed recorder");
    let resp = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .expect("GET /metrics");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "/metrics must return 200");
    assert!(
        headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/plain")),
        "/metrics must advertise text/plain"
    );
    assert!(
        body.contains("# TYPE http_requests_total counter"),
        "must expose http_requests_total in exposition; got: {body}"
    );
}

/// A request must bump the per-route/status counter.
#[tokio::test]
async fn http_metrics_request_counter_increments() {
    let server = TestServer::new().await;
    // Anonymous GET /healthz — a public route that needs no login.
    let before = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let before_count = count_http_requests(&before, "/healthz");

    // Fire one request to /healthz (and the scrape of /metrics
    // itself records an entry, but we only diff the /healthz row).
    let _ = server
        .client()
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .unwrap();

    let after = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let after_count = count_http_requests(&after, "/healthz");
    assert!(
        after_count > before_count,
        "http_requests_total{{route=/healthz}} must increase after a request; \
         before={before_count} after={after_count}"
    );
}

/// METRICS_ENABLED=false must mean /metrics is NOT registered.
#[tokio::test]
async fn http_metrics_disabled_when_flag_off() {
    let server = TestServer::new_metrics_disabled().await;
    let resp = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "/metrics must be unregistered when METRICS_ENABLED=false"
    );
}

/// Domain metric: creating a transaction must bump
/// `postings_created_total` (`o4-metrics-endpoint`).
#[tokio::test]
async fn http_metrics_postings_counter_increments_on_create() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("nina", "nina@example.com", "correct horse battery staple")
        .await;
    let pool = server.db().pool();

    // Create a ledger so a chart of accounts exists.
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.as_str())
        .form(&[
            ("name", "Metrics Ledger"),
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
        .expect("Location header")
        .to_string();
    let ledger_id = uuid::Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap();
    let (cash_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Cash on Hand' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sales_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Sales Revenue' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let before = scrape_counter(&server, "postings_created_total").await;

    // POST a balanced transaction through the HTTP endpoint so
    // the handler's counter path runs.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/transactions/new",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie.as_str())
        .form(&[
            ("date", "2026-08-10"),
            ("description", "Lemonade sale"),
            ("lines[0][account_id]", cash_id.to_string().as_str()),
            ("lines[0][amount]", "10.00"),
            ("lines[0][direction]", "DEBIT"),
            ("lines[1][account_id]", sales_id.to_string().as_str()),
            ("lines[1][amount]", "10.00"),
            ("lines[1][direction]", "CREDIT"),
        ])
        .send()
        .await
        .unwrap();
    let status = resp.status();
    if status != reqwest::StatusCode::SEE_OTHER {
        let body = resp.text().await.unwrap();
        panic!("transaction create should redirect on success; status={status} body={body:?}");
    }
    assert_eq!(
        status,
        reqwest::StatusCode::SEE_OTHER,
        "transaction create should redirect on success"
    );

    let after = scrape_counter(&server, "postings_created_total").await;
    assert!(
        after > before,
        "postings_created_total must increase after creating a 2-posting \
         transaction; before={before} after={after}"
    );
}

/// Metrics must not leak any request data (privacy: no cookie,
/// no query string, no transaction content).
#[tokio::test]
async fn http_metrics_no_secret_leakage() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("margo", "margo@example.com", "correct horse battery staple")
        .await;
    let body = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    for forbidden in [
        "oa_session",
        "margo@example.com",
        "correct horse battery staple",
        "?q=",
    ] {
        assert!(
            !body.contains(forbidden),
            "/metrics must not leak {forbidden:?}; got: {body}"
        );
    }
}

/// Count occurrences of `http_requests_total{...route="<route>"...}`.
fn count_http_requests(exposition: &str, route: &str) -> usize {
    exposition
        .lines()
        .filter(|line| {
            line.starts_with("http_requests_total")
                && line.contains(&format!("route=\"{route}\""))
                && !line.starts_with("#")
        })
        .fold(0usize, |acc, line| {
            let value = line.rsplit(' ').next().unwrap_or("0");
            acc + value.parse::<usize>().unwrap_or(0)
        })
}

/// Scrape `/metrics` and return the current value of a single
/// unlabeled counter, or 0 if it isn't in the exposition yet.
async fn scrape_counter(server: &TestServer, metric: &str) -> u64 {
    let body = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .expect("GET /metrics")
        .text()
        .await
        .expect("body");
    body.lines()
        .find(|line| line.starts_with(metric) && !line.starts_with('#'))
        .and_then(|line| line.rsplit(' ').next())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}
