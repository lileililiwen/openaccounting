# Sign Session Cookies with APP_SECRET

## Why

`app_secret` is collected at startup (`src/lib.rs:96-103`) but unused
(`#[allow(dead_code)]` at `lib.rs:88`). If the session store is compromised
(debug SQL access, leaked DB backup), an attacker can mint session rows.
Signing the cookie with HMAC gives tamper detection.

## What Changes

- Enable the `signed` feature on `tower-sessions`.
- Configure a `Key` derived from `APP_SECRET`.
- Invalidate sessions whose signature does not verify.

## Capabilities

### New Capabilities

- `signed-cookies`: HMAC-signed session cookies.

## Impact

**Modified files:**
- `Cargo.toml` — enable feature.
- `src/lib.rs` — pass the key to the session layer.
