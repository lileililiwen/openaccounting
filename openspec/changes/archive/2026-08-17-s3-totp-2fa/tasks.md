## 1. Testing

- [x] 1.1 Unit: `totp_code_verifies_against_secret` — generate code, verify true.
- [x] 1.2 Unit: `totp_old_code_rejected_after_use` — replay test.
- [x] 1.3 Unit: `totp_recovery_code_one_shot` — consume, second consume fails.
- [x] 1.4 HTTP: `http_2fa_enroll_success` — full enroll flow.
- [x] 1.5 HTTP: `http_2fa_login_requires_code` — password OK, no code = no session.
- [x] 1.6 HTTP: `http_2fa_disable_requires_both_factors` — disable with only password fails.
- [x] 1.7 HTTP: `http_2fa_secret_never_leaked` — scrape responses, ensure secret bytes absent.

## 2. Implementation

- [x] 2.1 `Cargo.toml` — add `totp-rs = "6"`, `qrcodegen = "1.8"`, `hkdf`, `rand`, `sha2`, uuid v5 feature.
- [x] 2.2 `migrations/0029_add_user_totp.sql` — `user_totp` + `recovery_codes` tables + indexes.
- [x] 2.3 `src/auth/totp.rs` — secret gen, verify, encrypt/decrypt with HKDF, recovery codes, QR SVG.
- [x] 2.4 `src/handlers/account_security.rs` — enroll/confirm/disable/regen.
- [x] 2.5 `src/auth/handlers.rs` — 2FA step partial + submit handler, login_submit gates on enrollment.
- [x] 2.6 `templates/account/security.html` + `templates/auth/login_2fa.html`.
- [x] 2.7 `src/lib.rs` — register new routes; `totp_cipher` field on `AppState`.

## 3. Validation

- [x] 3.1 `openspec validate s3-totp-2fa` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support` green; 8 unit + 6 HTTP tests added.
- [x] 3.5 `openspec archive s3-totp-2fa`.