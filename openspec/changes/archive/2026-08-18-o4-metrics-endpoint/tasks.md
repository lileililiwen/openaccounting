## 1. Testing

- [x] 1.1 HTTP: `http_metrics_endpoint_returns_200_prometheus_format` — seeds the recorder with a /healthz hit, then asserts 200, `text/plain` content-type, and a `# TYPE http_requests_total counter` line.
- [x] 1.2 HTTP: `http_metrics_request_counter_increments` — scrapes `/metrics`, fires a request to `/healthz`, re-scrapes, and asserts the `http_requests_total{route="/healthz"}` series increased.
- [x] 1.3 HTTP: `http_metrics_disabled_when_flag_off` — `TestServer::new_metrics_disabled()` builds the router with `metrics_enabled=false`; `/metrics` returns 404 (route NOT registered).
- [x] 1.4 Unit: `metrics_recorder_handles_concurrent_updates` — 8 threads × 100 `postings_created(1)` increments, then parses the rendered exposition and asserts exactly 800.
- [x] 1.5 HTTP: `http_metrics_postings_counter_increments_on_create` — POSTs a balanced 2-posting transaction through the handler and asserts `postings_created_total` increased.
- [x] 1.6 HTTP: `http_metrics_no_secret_leakage` — asserts the exposition never contains the session cookie, the user email, the password, or a query string.

## 2. Implementation

- [x] 2.1 `Cargo.toml` — added `metrics = "0.24.6"` and `metrics-exporter-prometheus = "0.18.3"`.
- [x] 2.2 `src/observability/mod.rs` + `src/observability/metrics.rs` — idempotent global recorder install (first `OnceLock` wins; integration tests share one process), `render()` for the scrape, the axum `http_metrics` middleware, the `metrics_page` handler, and the domain counter helpers (`postings_created`, `reconciliation_completed`, `backup_completed`, `failed_login`).
- [x] 2.3 Emit metrics in handlers — `src/handlers/transactions.rs` (postings created), `src/handlers/reconciliation.rs` (reconciliations completed), `src/handlers/backups.rs` (backups completed), `src/auth/rate_limit.rs` (failed logins).
- [x] 2.4 `src/lib.rs` — `/metrics` registered on the public router, gated by `config.metrics_enabled`; the `http_metrics` middleware is applied to the whole router. `AppConfig` gained `metrics_enabled` + `with_metrics_enabled`.
- [x] 2.5 `src/config.rs` — `METRICS_ENABLED` env var (default true; `false`/`0` disables).

## 3. Validation

- [x] 3.1 `openspec validate o4-metrics-endpoint`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support` — only pre-existing warnings.
- [x] 3.4 `cargo test --features test-support` — full integration suite green (143 tests, up from 138).
- [x] 3.5 `openspec archive o4-metrics-endpoint`.

## 4. Bug fixed while testing

The metrics domain-counter test POSTed a transaction through `/transactions/new` for the first time over HTTP and exposed a pre-existing 500: the `create` handler's `INSERT ... RETURNING` omitted `template_id` (added by the `recurring-transactions` spec), so `sqlx` failed with "no column found for name: template_id" for EVERY transaction created via the UI/API. Fixed by adding `template_id` to the RETURNING list (`src/handlers/transactions.rs`).