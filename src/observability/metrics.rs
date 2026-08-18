//! Prometheus recorder, HTTP middleware, and domain counters.
//!
//! [`install`] is idempotent — the `metrics` crate allows only
//! one global recorder per process, and integration tests spin
//! up many `TestServer`s in one binary. The first caller wins;
//! every later `TestServer` shares the same recorder, which is
//! exactly what a scraped `/metrics` endpoint wants.

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

/// Latency buckets for the HTTP histogram: 1 ms → 10 s.
const HTTP_LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus recorder. Idempotent; safe to
/// call from every `TestServer`/process startup.
pub fn install() {
    let _ = HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .set_buckets(HTTP_LATENCY_BUCKETS)
            .expect("valid histogram buckets")
            .install_recorder()
            .expect("install prometheus recorder")
    });
}

/// Whether the recorder has been installed at least once.
pub fn is_installed() -> bool {
    HANDLE.get().is_some()
}

/// Render the full Prometheus exposition snapshot.
///
/// Callers must NOT render while holding the tokio runtime
/// blocked on the DB; the recorder is lock-free reads.
pub fn render() -> String {
    match HANDLE.get() {
        Some(handle) => handle.render(),
        None => String::new(),
    }
}

/// `GET /metrics` handler — Prometheus exposition text.
/// Public (no auth), rate-limit-exempt.
pub async fn metrics_page() -> axum::response::Response {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; version=0.0.4"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    (axum::http::StatusCode::OK, headers, render()).into_response()
}

/// Axum middleware recording request counts + latency.
///
/// The `route` label uses axum's [`MatchedPath`] (the
/// `{id}`-style pattern), NOT the raw URI, so Prometheus series
/// stay bounded and cardinally safe. No user-identifying data
/// (cookie, IP, query string) is ever recorded.
pub async fn http_metrics(req: Request, next: Next) -> Response {
    let started = std::time::Instant::now();
    let method = req.method().as_str().to_owned();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| req.uri().path().to_owned());

    let resp = next.run(req).await;
    let status = resp.status().as_u16();

    let status = status.to_string();
    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "route" => route.clone(),
        "status" => status.clone(),
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "route" => route,
        "status" => status,
    )
    .record(started.elapsed().as_secs_f64());
    resp
}

/// Increment `postings_created_total` by `n`.
pub fn postings_created(n: u64) {
    metrics::counter!("postings_created_total").increment(n);
}

/// Increment `reconciliations_total`.
pub fn reconciliation_completed() {
    metrics::counter!("reconciliations_total").increment(1);
}

/// Increment `backups_total`.
pub fn backup_completed() {
    metrics::counter!("backups_total").increment(1);
}

/// Increment `failed_logins_total`.
pub fn failed_login() {
    metrics::counter!("failed_logins_total").increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_buckets_span_one_ms_to_ten_seconds() {
        assert_eq!(HTTP_LATENCY_BUCKETS.first(), Some(&0.001));
        assert_eq!(HTTP_LATENCY_BUCKETS.last(), Some(&10.0));
        assert!(HTTP_LATENCY_BUCKETS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn metrics_recorder_handles_concurrent_updates() {
        // The recorder is a global; this test must run in the
        // lib test binary, which is a separate process from the
        // integration binary, so it can install its own recorder.
        install();

        const THREADS: u64 = 8;
        const PER_THREAD: u64 = 100;

        std::thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|| {
                    for _ in 0..PER_THREAD {
                        postings_created(1);
                    }
                });
            }
        });

        let exposition = render();
        let observed = exposition
            .lines()
            .find(|line| line.starts_with("postings_created_total") && !line.starts_with('#'))
            .and_then(|line| line.rsplit(' ').next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        assert_eq!(
            observed,
            THREADS * PER_THREAD,
            "concurrent increments must all be visible; exposition={exposition}"
        );
    }
}
