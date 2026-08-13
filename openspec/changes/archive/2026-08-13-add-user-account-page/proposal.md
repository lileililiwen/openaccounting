# Add user account page

## Why

After registering, the only way for a user to see or change anything
about their account is to log out and register a new one. There is no
profile view, no password-change form, and no self-service surface at
all. This is a basic usability gap and a security liability: users who
suspect a compromised password have no way to rotate it without
involving an admin (and there is no admin yet either).

This change adds a self-service `/account` page so any authenticated
user can see their identity and rotate their password.

## What Changes

- New handler module `src/handlers/account.rs` with:
  - `GET  /account`              — render profile + change-password form
  - `POST /account/password`     — verify current password, set new
- New Askama template `templates/account.html` (server-rendered).
- New auth helper `change_password(pool, user_id, current, new)` in
  `src/auth/mod.rs`. The helper returns `Result<(), AppError>` with
  distinct `Unauthorized` (wrong current) vs `Validation` (weak new
  password) variants so the handler can render a clear error.
- Navigation: add an "Account" link to `partials/_nav.html`, visible
  on every authenticated page.
- New unit tests in `src/handlers/account.rs::tests` covering:
  - empty current password → 400 Validation
  - wrong current password   → 401 Unauthorized
  - new password too short   → 400 Validation
  - new passwords don't match → 400 Validation
  - happy path: rehash + update + redirect → 303
- New HTTP smoke test in `tests/http/account.rs` exercising the full
  flow through the running binary against a fresh DB.

## Capabilities

### New Capabilities

- `account` — the user-self-service capability: profile view and
  password rotation. New spec: `specs/account/spec.md`.

### Modified Capabilities

- None.

## Impact

- **New files:** `src/handlers/account.rs`,
  `templates/account.html`, `tests/http/account.rs`,
  `openspec/specs/account/spec.md` (delta on archive).
- **Modified files:** `src/main.rs` (two new routes), `src/auth/mod.rs`
  (one new helper), `templates/partials/_nav.html` (one new link).
- **Database:** none. We reuse the existing `users` table and its
  `hashed_password` column.
- **Sessions:** a successful password change does **not** invalidate
  existing sessions. That is a follow-up; for v0.1, an attacker who
  already has a session keeps it. We will revisit this when we add the
  `session_version` column as part of the admin change.
- **Backwards compatibility:** all new routes are additive.

## Non-Goals (v0.1)

- Email change (requires verification flow; future change).
- Display-name change (small enough to add later, but not now).
- Two-factor authentication.
- "Remember me" / persistent login state.
- Listing or revoking the user's active sessions.
- Admin tools for managing users (handled by the follow-up
  `add-admin-role-and-dashboard` change).
