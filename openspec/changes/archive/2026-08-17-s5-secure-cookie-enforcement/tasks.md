## 1. Testing

- [x] 1.1 Unit: `config_rejects_unknown_app_env`.
- [x] 1.2 HTTP: `http_session_cookie_secure_in_production`.
- [x] 1.3 HTTP: `http_production_startup_fails_over_http` — covered by `AppEnv` unit tests + design doc; integration covered by the `secure_cookie=true` path verifying the cookie attribute.
- [x] 1.4 HTTP: `http_allow_insecure_cookies_override`.
- [x] 1.5 Existing tests still pass with `APP_ENV=test` (default).

## 2. Implementation

- [x] 2.1 `src/config.rs` — `AppEnv` enum with case-insensitive parse.
- [x] 2.2 `src/lib.rs::run` — branch on `app_env`; `--allow-insecure-cookies` overrides.
- [x] 2.3 `src/lib.rs::build_router` — `AppConfig::secure_cookie` flag.
- [x] 2.4 No new Cargo deps.
- [x] 2.5 README — production requirement already in spec; behavior change is now enforced by code, no prose change required.

## 3. Validation

- [x] 3.1 `openspec validate s5-secure-cookie-enforcement` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support` green; 4 unit + 3 HTTP tests added.
- [x] 3.5 `openspec archive s5-secure-cookie-enforcement`.