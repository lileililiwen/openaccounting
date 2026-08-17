## 1. Testing

- [x] 1.1 HTTP: `http_signed_cookie_tampered_rejected`.
- [x] 1.2 HTTP: `http_signed_cookie_valid_accepted`.
- [x] 1.3 HTTP: `http_key_rotation_accepts_prev_key`.

## 2. Implementation

- [x] 2.1 `src/auth/cookie_signer.rs` — HMAC-SHA256 with multi-key rotation; constant-time MAC compare.
- [x] 2.2 `src/lib.rs::build_router` — derives the signer from `AppConfig` and installs the middleware as the outermost layer.
- [x] 2.3 `src/config.rs` — `APP_SECRET_PREVIOUS` is parsed and propagated through `Config` → `AppConfig`.

## 3. Validation

- [x] 3.1 `openspec validate s7-signed-cookies` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support` green; 6 unit + 3 HTTP tests added.
- [x] 3.5 `openspec archive s7-signed-cookies`.