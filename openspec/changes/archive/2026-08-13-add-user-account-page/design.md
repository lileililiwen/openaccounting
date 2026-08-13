# Add user account page — Design

## Context

The bootstrap change shipped users + Argon2id + sessions but no
self-service surface. Every authenticated user today can manage their
ledgers but nothing about themselves.

The Account page lives at `/account` and is reachable from the
top-right nav (`@username` becomes a link to `/account`).

## Routes

```
GET   /account            200   render profile + password form
POST  /account/password   303   success → redirect to /account
                          400   validation error (weak new password, mismatch)
                          401   wrong current password
                          404   not authenticated (login_required)
```

Both routes are inside the protected sub-router so the `login_required!`
layer applies.

## Template

```
templates/account.html
├── Profile card
│   ├── Username  (read-only)
│   ├── Email     (read-only)
│   ├── Display name (read-only, shows "—" when None)
│   ├── Member since  (formatted YYYY-MM-DD)
│   └── Last updated  (formatted YYYY-MM-DD HH:MM)
└── Change-password card
    ├── current_password  (type=password, required)
    ├── new_password      (type=password, required, minlength=8)
    ├── confirm_password  (type=password, required, minlength=8)
    └── Submit
```

The page inherits from `base.html` and includes `partials/_nav.html`.
On a successful POST, the same template is re-rendered with a
`flash = "Password updated."` banner at the top. On error, the
form is re-rendered with the user's input preserved and an `error`
banner above the relevant field.

## Auth helper

```rust
// src/auth/mod.rs
pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    current: &str,
    new: &str,
) -> Result<(), AppError> {
    // 1. Load user (need hashed_password for verification).
    // 2. Verify `current` against stored hash → if false, AppError::Unauthorized.
    // 3. Validate `new` (≥ 8 chars) → if false, AppError::Validation.
    // 4. Hash `new` with Argon2id.
    // 5. UPDATE users SET hashed_password = $1, updated_at = now() WHERE id = $2.
    Ok(())
}
```

Distinct error variants let the handler pick the right HTTP code and
message without leaking whether the user exists.

## Nav update

`partials/_nav.html` currently renders `@{{ username }}` as plain
text. We wrap it in an anchor:

```html
<a href="/account" class="...">@{{ username }}</a>
```

No other nav changes. The mobile sub-nav is untouched.

## Tests

Following the spec-first rule (`Agents.md §2.4`), tests come first.

### Unit (`src/handlers/account.rs::tests`)

These exercise the `change_password` helper directly via a
`TestDb` (we'll add a real one in a later change; for v0.1 we use
the live `DATABASE_URL` from env and isolate by username).

- `change_password_rejects_empty_current`
- `change_password_rejects_wrong_current`
- `change_password_rejects_too_short_new`
- `change_password_rejects_mismatched_new_and_confirm`  (handler-level)
- `change_password_succeeds_and_persists`               (re-login with new)
- `change_password_with_same_value_as_current_succeeds` (no rule against reuse)

### HTTP (`tests/http/account.rs`)

- `get_account_redirects_when_logged_out`
- `get_account_renders_profile_when_logged_in`
- `post_password_with_wrong_current_returns_401`
- `post_password_with_mismatched_new_returns_400`
- `post_password_happy_path_returns_303_and_new_password_works`

The HTTP tests run against a live binary on `OA_TEST_PORT` (default
`3001`) with a per-test fresh database.

## Risks / Trade-offs

- **R1: Existing sessions are not invalidated on password change.**
  → Acceptable for v0.1; called out in the proposal. We will fix it
  when we add the admin change.
- **R2: A user could change their password to the same value.**
  → Acceptable; the schema doesn't forbid it and it's a legitimate
  use case (some people periodically "re-confirm" their password).
- **R3: No rate limiting on `/account/password`.** → Acceptable; the
  rate-limiting concern is mostly about *failed* logins, which we
  already have at the session level. Password change is gated behind
  an authenticated session. A future change will add a per-IP
  backoff.

## Migration Plan

Greenfield addition. No data migration. The bootstrap schema already
has `users.hashed_password` and `users.updated_at`.

## Open Questions

- **Q1:** Should we email the user when their password changes?
  → Defer. v0.1 has no email infra. Will revisit when we add email.
- **Q2:** Should we add a "log out everywhere else" button?
  → Yes, but as part of the admin change when we add session
  versioning.
