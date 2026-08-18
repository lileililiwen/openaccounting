# Prometheus Metrics Endpoint

## Why

No `/metrics` exists. The `TraceLayer` (`src/lib.rs:553`) logs each
request but exposes no counters. Production observability — request rate,
latency percentiles, queue depths — requires either Prometheus or an
OpenTelemetry exporter.

## What Changes

- New `GET /metrics` returning the Prometheus exposition format.
- Counters for HTTP requests by route, status, method.
- Histograms for request latency.
- Custom counters: postings_created_total, reconciliations_total,
  backups_total.
- `tower-http::metrics` for the framework level; custom for domain.

## Capabilities

### New Capabilities

- `metrics`: Prometheus metrics.

## Impact

**New files:**
- `src/observability/mod.rs`, `src/observability/metrics.rs`.
- `tests/http/metrics.rs`.

**Modified files:**
- `src/lib.rs` — `/metrics` route.
- `Cargo.toml` — `metrics`, `metrics-exporter-prometheus`.
