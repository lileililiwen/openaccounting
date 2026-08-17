# Enforce Strong Password Policy with Breach Check

## Why

Current rules: `len ≥ 8` only (`src/auth/handlers.rs:105`,
`src/auth/mod.rs:150`). NIST 800-63B now recommends length + breach-list
check, not complexity. Eight-character passwords are crackable in hours
against modern hardware. The system also stores passwords that may match
known breaches silently.

## What Changes

- Raise minimum length to 12.
- Reject any password that appears in the HIBP Pwned Passwords top
  100k common list (bundled as a static file; offline check, no network).
- Optional HIBP API check (`--features hibp-online`) using k-anonymity
  (SHA-1 prefix sent, full hash never leaves the box).
- Apply on registration, password change, and account-creation-by-admin.

## Capabilities

### New Capabilities

- `password-strength`: Minimum-length + breach-aware password policy.

## Impact

**New files:**
- `static/security/common_passwords.txt` (~100k entries, gzipped).
- `src/auth/password.rs` — add `validate_strength`.
- `tests/unit/password_strength.rs`.

**Modified files:**
- `src/auth/handlers.rs::register_submit` — call validate.
- `src/auth/mod.rs::change_password` — call validate.
- `Cargo.toml` — optional `reqwest` already present, optional `flate2`.
