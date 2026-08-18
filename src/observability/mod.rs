//! Observability: Prometheus metrics (`o4-metrics-endpoint`).
//!
//! Two layers:
//!
//! * Framework level — every HTTP request records a counter
//!   `http_requests_total{route,method,status}` and a latency
//!   histogram `http_request_duration_seconds` via a thin
//!   middleware (tower-http 0.6 has no general MetricsLayer;
//!   only `InFlightRequests`).
//! * Domain level — handlers increment
//!   `postings_created_total`, `reconciliations_total`,
//!   `backups_total`, and `failed_logins_total`.
//!
//! The recorder is installed once per process via
//! `metrics_exporter_prometheus`; the `/metrics` route renders
//! the snapshot on demand (no HTTP listener of its own).

pub mod metrics;
