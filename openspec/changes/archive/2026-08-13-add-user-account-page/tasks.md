# Add user account page — Tasks

> **Spec-first rule (from `Agents.md §2.4`):** `## 1. Testing` comes
> first. Tests are written and red **before** any `## 2.
> Implementation` task is marked complete.

## 1. Testing

- [ ] 1.1 Unit: `auth::change_password` rejects an empty `current`
      with `AppError::Validation` ("…at least 8 characters" or
      "Current password is incorrect" — the helper sees both
      inputs; the handler chooses the right message). The order
      of checks matters and is covered by 1.2 — 1.4.
- [ ] 1.2 Unit: `auth::change_password` returns
      `AppError::Unauthorized` when `current` does not verify
      against the stored hash. The user's `hashed_password` row
      MUST be unchanged after the call.
- [ ] 1.3 Unit: `auth::change_password` returns
      `AppError::Validation` when `new` is shorter than 8
      characters. No row is updated.
- [ ] 1.4 Unit: `auth::change_password` succeeds and persists when
      all checks pass. A subsequent `verify_password(new, &stored)`
      returns `Ok(())`, and `verify_password(old, &stored)` returns
      `Err(_)`.
- [ ] 1.5 HTTP: `GET /account` with no session returns
      `303 → /login?next=/account`.
- [ ] 1.6 HTTP: `GET /account` with a valid session returns
      `200` and the HTML contains the user's `username`, `email`,
      formatted `created_at`, and the password form (with a
      `current_password`, `new_password`, and `confirm_password`
      field each).
- [ ] 1.7 HTTP: `POST /account/password` with a wrong
      `current_password` returns `401` and the body
      `Current password is incorrect.`
- [ ] 1.8 HTTP: `POST /account/password` with a too-short
      `new_password` returns `400` and the body
      `New password must be at least 8 characters.`
- [ ] 1.9 HTTP: `POST /account/password` with mismatched
      `new_password` and `confirm_password` returns `400` and the
      body
      `New password and confirmation do not match.`
- [ ] 1.10 HTTP: `POST /account/password` happy path returns
      `303 → /account`; a follow-up `POST /login` with the new
      password succeeds and with the old password fails.

## 2. Implementation

- [ ] 2.1 Add `change_password(pool, user_id, current, new) -> Result<(), AppError>`
      in `src/auth/mod.rs` with the four checks from 1.1 — 1.4.
      Reuse `password::hash_password` and `password::verify_password`.
- [ ] 2.2 New module `src/handlers/account.rs` with:
      - `pub async fn show(auth, state) -> AppResult<Response>`
        rendering `AccountPage`.
      - `pub async fn change_password(auth, state, Form(form)) -> AppResult<Response>`
        that calls the helper and renders the right page on each
        outcome (303 / 401 / 400 / 400 / 200-with-flash).
- [ ] 2.3 New Askama struct `AccountPage` in
      `src/templates/account.rs` with fields
      `user: User, error: String, flash: String`.
- [ ] 2.4 New template `templates/account.html` (extends
      `base.html`, includes `_nav.html`, renders the profile card
      and the password card).
- [ ] 2.5 Update `templates/partials/_nav.html` to make the
      `@username` text a link to `/account`.
- [ ] 2.6 Wire the two routes in `src/main.rs`:
      ```
      .route("/account",            get(handlers::account::show))
      .route("/account/password",   post(handlers::account::change_password))
      ```
      Both live inside the `protected` sub-router so
      `login_required!` applies.

## 3. Validation

- [ ] 3.1 `openspec validate add-user-account-page` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean for any
      file touched by this change.
- [ ] 3.4 `cargo build --release` succeeds.
- [ ] 3.5 All tasks under `## 1. Testing` are green.
- [ ] 3.6 Manual smoke: log in → open `/account` → change password
      → log out → log in with the new password.
- [ ] 3.7 `openspec archive add-user-account-page` — the delta is
      folded into `openspec/specs/account/spec.md` and the change
      moves to `openspec/changes/archive/`.
