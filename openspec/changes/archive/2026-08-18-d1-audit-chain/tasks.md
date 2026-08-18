## 1. Testing

- [x] 1.1 Unit: `hash_chain_links_correctly` — covered by `hash_changes_with_prev_hash`, `canonical_bytes_are_deterministic`, and `null_vs_present_distinguished` in `src/audit/chain.rs`.
- [x] 1.2 Unit: `tampered_row_detected` — covered by `http_audit_verify_tampered_reports_break` (UPDATE a row's `new_value`, then `verify()` reports the break).
- [x] 1.3 HTTP: `http_audit_verify_clean_200_ok` — seed rows via `audit::log`, `verify()` returns zero break points; tail hash is 64 hex chars.
- [x] 1.4 HTTP: `http_audit_verify_tampered_reports_break` — tamper via SQL, `verify()` reports a SHA256 / prev_hash mismatch.
- [x] 1.5 DB: `audit_update_permission_denied_for_app_role` — **deferred**: the project has no separate DB app role; every connection is the table owner (verified: no `CREATE ROLE`/`GRANT` in any migration). Issuing a `REVOKE` would break the app itself. A future change introducing a least-privilege app role should add this REVOKE. Documented in tasks.md, not silently dropped.
- [x] 1.6 HTTP: `http_audit_verify_admin_endpoint` — non-admin gets 403; admin (re-logged-in after role change, since sessions cache the role) gets 200 with chain status.
- [x] 1.7 Worker: `audit_anchor_writes_tail_hash_line` — `run_once` appends `ISO8601 <hex>` to `AUDIT_ANCHOR_FILE`.

## 2. Implementation

- [x] 2.1 `migrations/0037_add_audit_chain.sql` — adds `prev_hash BYTEA` + `hash BYTEA` + the `(created_at, id)` chain-order index. The backfill is done by the application (`ensure_backfilled`, called from `run()` and `TestServer::new()`) because the canonical serialization lives in Rust and would drift from a PL/pgSQL copy. REVOKE deferred with 1.5.
- [x] 2.2 `src/audit/chain.rs` — `canonical_bytes` (deterministic, length-prefixed), `compute_hash` = SHA256(prev || content), `log_hashed` (appends under `pg_advisory_xact_lock(hashtext('audit_entries'))` so concurrent appends can't read the same tail), `ensure_backfilled`, `verify`, `latest_hash_hex`. `src/audit.rs::log` now delegates to `log_hashed` and passes `created_at` explicitly so the hash covers it.
- [x] 2.3 `src/handlers/admin_audit_verify.rs` + `templates/admin/audit_verify.html` — admin-only `GET /admin/audit/verify` shows intact/broken status, entry count, tail hash, and break-point rows.
- [x] 2.4 Daily anchor worker — `src/workers/audit_anchor.rs` (spawned in `run()`), appends the tail hash to `AUDIT_ANCHOR_FILE` (default `./data/audit-anchor.log`) daily with fsync.

## 3. Validation

- [x] 3.1 `openspec validate d1-audit-chain`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support` — no new warnings.
- [x] 3.4 `cargo test --features test-support` — full integration suite green (147 tests, up from 143).
- [x] 3.5 `openspec archive d1-audit-chain`.