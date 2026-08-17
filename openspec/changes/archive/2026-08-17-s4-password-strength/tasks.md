## 1. Testing

- [x] 1.1 Unit: `validate_accepts_strong_password`.
- [x] 1.2 Unit: `validate_rejects_short_password`.
- [x] 1.3 Unit: `validate_rejects_common_password`.
- [x] 1.4 Unit: `validate_scrubs_password_in_error` — log assertion.
- [x] 1.5 HTTP: `http_register_rejects_short_password`.
- [x] 1.6 HTTP: `http_register_rejects_common_password`.
- [x] 1.7 HTTP: `http_change_password_enforces_policy`.

## 2. Implementation

- [x] 2.1 Vendored `data/security/common_passwords.txt` (curated subset of SecLists + HIBP top 100).
- [x] 2.2 `src/auth/password.rs::validate_strength` — length ≥ 12 + deny-list check, `OnceLock`-cached `HashSet`.
- [x] 2.3 `src/auth/handlers.rs::register_submit` integration.
- [x] 2.4 `src/auth/mod.rs::change_password` integration.
- [x] 2.5 Optional `hibp-online` feature behind `cfg(feature = "hibp-online")` (fail-open on API error).
- [x] 2.6 README and Agents.md reviewed — no explicit policy mention to update.

## 3. Validation

- [x] 3.1 `openspec validate s4-password-strength` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support` green; 8 unit + 3 HTTP tests added.
- [x] 3.5 `openspec archive s4-password-strength`.