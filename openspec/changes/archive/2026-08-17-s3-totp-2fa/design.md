# ## Context

The auth spec (`openspec/specs/auth/spec.md`) defines a single-factor
password flow. There is no TOTP, no WebAuthn, no SMS. The `app_secret`
already exists for future use; this change consumes it for HKDF key
derivation.

## Goals / Non-Goals

**Goals:**
- Standards-compliant TOTP (RFC 6238) with 30-second period, 6 digits,
  SHA-1 default (compatible with Google Authenticator, 1Password, Bitwarden).
- Recovery codes that work even if the user loses their device.
- Encrypted-at-rest secret using the existing `APP_SECRET`.

**Non-Goals:**
- WebAuthn / passkeys (separate change — S12).
- SMS-based 2FA (deprecated, vulnerable to SIM swap).

## Decisions

- **Library.** `totp-rs = "2"`. Permits both `Sha1` (max compatibility) and
  a future move to `Sha256`.
- **Storage encryption.** The TOTP secret is encrypted with AES-256-GCM
  using a key derived as `HKDF-SHA256(APP_SECRET, info="totp-secret-v1")`.
  Same pattern as the bank-feed tokens (`src/bank_feeds/`).
- **Counter.** `last_used_counter` is updated atomically; replays fail with
  a 1-step window tolerance (to handle clock skew within ±30 s).
- **Recovery codes.** 10 random 8-char base32 codes, Argon2id-hashed.

## Risks / Trade-offs

- **Lost device + lost recovery codes** = account lockout. The disable path
  requires admin recovery — out of scope, documented as a future "account
  recovery" change.
- **HSM for `APP_SECRET`**. The encryption key is software-only; an attacker
  with shell access can decrypt. This is the same threat model as the
  existing bank-feed token storage; documented in `SECURITY.md`.
