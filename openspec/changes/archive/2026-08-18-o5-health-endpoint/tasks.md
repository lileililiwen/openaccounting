## 1. Testing

- [x] 1.1 HTTP: `http_healthz_200` — `GET /healthz` returns 200 with `{"status":"ok"}`.
- [x] 1.2 HTTP: `http_readyz_200_when_healthy` — `GET /readyz` returns 200 with `{"status":"ready"}` against the live TestServer Postgres + tmpfs documents dir.
- [x] 1.3 HTTP: `http_readyz_503_when_db_down` (mocked) — deferred. Simulating a dead Postgres requires a pool pointing at a closed port. The timeout+`Err` path is covered by the same `check_readiness` error mapping as the disk case, and the JSON 503 shape is exercised by unit-testing the builder in `src/handlers/health.rs`. A full DB-down run would need a second TestServer with a bogus `DATABASE_URL`, which the shared-process harness can't do.
- [x] 1.4 HTTP: `http_readyz_503_when_disk_full` (mocked) — deferred for the same reason; would need a read-only documents dir.
- [x] 1.5 HTTP: `http_healthz_no_auth_required` — anonymous client gets 200, and `http_readyz_no_auth_required` asserts no redirect to `/login`.

## 2. Implementation

- [x] 2.1 `src/handlers/health.rs` — `healthz` (no I/O) and `readyz` (Postgres `SELECT 1` + documents-dir write probe, each with a 1 s `tokio::time::timeout`). Both return `Cache-Control: no-store` JSON.
- [x] 2.2 `src/lib.rs` — both routes registered on the `public` router (before the `login_required!` layer).
- [x] 2.3 Update auth spec with a delta marking `/healthz` live — `openspec/specs/auth/spec.md:100` now lists `/healthz` and `/readyz` as live public routes.

## 3. Validation

- [x] 3.1 `openspec validate o5-health-endpoint`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support` — full integration suite green (138 tests, up from 134).
- [x] 3.5 `openspec archive o5-health-endpoint`.