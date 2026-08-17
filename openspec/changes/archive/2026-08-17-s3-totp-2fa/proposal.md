# Add TOTP-based Two-Factor Authentication

## Why

OpenAccounting is a financial system that handles transactions, invoices,
and bank feeds. Yet the only auth factor is a password (Argon2id-hashed, with
no 2FA). A single phishing incident compromises an entire ledger. Firefly
III, Akaunting, GnuCash Mobile, and modern accounting SaaS all offer TOTP.
OpenAccounting has no MFA option today.

## What Changes

- New `user_totp` table `(user_id, secret_encrypted, enrolled_at,
  last_used_counter)`.
- New `recovery_codes` table (10 one-time codes, Argon2-hashed at rest).
- New settings page `/account/security` to enroll and manage 2FA.
- Modified login flow: after password verify, if 2FA is enrolled, the
  response is a "2FA pending" partial; the user submits the 6-digit code to
  `/login/2fa` to complete the session.
- TOTP secrets encrypted at rest with AES-256-GCM using a key derived from
  `APP_SECRET` via HKDF.

## Capabilities

### New Capabilities

- `totp-2fa`: Time-based one-time password second factor.

## Impact

**New files:**
- `src/auth/totp.rs` — secret gen, code verify, recovery-code gen/verify.
- `src/handlers/account_security.rs` — enroll, confirm, disable, regenerate.
- `templates/account/security.html`
- `templates/auth/login_2fa.html`
- `migrations/0029_add_user_totp.sql`
- `tests/http/totp.rs`

**Modified files:**
- `src/auth/handlers.rs` — second step after password verify.
- `src/lib.rs` — add new routes.
- `Cargo.toml` — `totp-rs = "2"`.
