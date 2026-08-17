## 1. Testing

- [x] 1.1 Unit: `session_guard_defaults_match_spec` (30 min idle, 12 h absolute).
- [x] 1.2 Unit: `urlencode_passes_through_safe_chars`.
- [x] 1.3 HTTP: `http_session_idle_timeout_redirects_to_login`.
- [x] 1.4 HTTP: `http_session_absolute_timeout_redirects_to_login`.
- [x] 1.5 HTTP: `http_session_active_refresh_extends_lifetime`.

## 2. Implementation

- [x] 2.1 No migration needed — `created_at` / `last_seen_at` live in the session payload (the existing tower-sessions `session` table already has a generic `data bytea` column).
- [x] 2.2 `src/auth/session_timeout.rs` — `SessionGuard` config, `mark_authenticated` helper, `enforce_timeout` middleware (millisecond resolution so 1-second test windows work).
- [x] 2.3 `src/lib.rs::build_router_with_session_guard` — explicit guard parameter; `run()` falls back to env (`SESSION_IDLE_SECONDS`, `SESSION_ABSOLUTE_SECONDS`). Middleware installed via `axum::middleware::from_fn_with_state`.
- [x] 2.4 `login_submit` and `login_2fa_submit` call `mark_authenticated`; `login_page` renders the `expired=1` flash.

## 3. Validation

- [x] 3.1 `openspec validate s6-session-timeout` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support` green; 2 unit + 3 HTTP tests added.
- [x] 3.5 `openspec archive s6-session-timeout`.