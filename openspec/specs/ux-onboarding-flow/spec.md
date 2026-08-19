# ux-onboarding-flow Specification

## Purpose
A brand-new user must be able to register, reach ledger creation, and record their first transaction without getting stuck, misled, or locked out of a flow they just started. Found by a novice-user form submission test on 2026-08-19; evidence in `docs/ux-novice-form-test/*.png`.

## Requirements

### Requirement: Register Auto-Login

MUST sign the user in immediately after a successful registration and redirect them to the `next` target (default `/ledgers/new`); MUST NOT send the freshly registered user back to `/login` to re-enter credentials.

#### Scenario: Fresh registration

- **WHEN** a user completes the register form
- **THEN** a session is created and the user lands on `/ledgers/new` (observed: the app redirected to `/login?next=/ledgers/new`, see `05_after_register.png`).

### Requirement: Preserved Next Target

MUST preserve the `next` parameter across the register → login boundary so the user continues where the flow intended; if `next` is lost the fallback target SHALL be `/ledgers` (the ledger list).

#### Scenario: Login after registration

- **WHEN** a user registers (next = `/ledgers/new`) and then signs in from the plain `/login` page
- **THEN** the user is redirected to `/ledgers/new`, not the ledger list (observed: landed on `/ledgers`, losing the intended next step).

### Requirement: Inline Password Feedback

SHOULD show a live password-strength meter and SHOULD surface server-side password-rejection reasons (e.g. "common password") inline on the register form instead of relying only on browser validation and static helper text.

#### Scenario: Weak password

- **WHEN** a user types a short or common password
- **THEN** the form gives immediate inline feedback about length/strength in the UI language (observed: only the browser's localized tooltip appeared, see `04_register_short_pw.png`).

### Requirement: Login Error Preserves Email

SHOULD preserve the entered email and focus the password field after a failed login, so a typo only requires re-entering one field.

#### Scenario: Mistyped password

- **WHEN** a user submits a login with a wrong password
- **THEN** the email remains filled and the password field is focused (observed: both fields were cleared after "Invalid email or password", see `02_random_login_error.png`).

### Requirement: Jargon-Free Empty States

SHOULD phrase empty-state copy in everyday language; SHALL avoid requiring the user to know "debit" and "credit" before they have recorded their first transaction.

#### Scenario: First transactions

- **WHEN** a ledger has no transactions
- **THEN** the empty state says "Record your first income or expense" rather than "Record your first debit and credit" (see `20_txn_list.png`).

### Requirement: Append-Only Enable Confirmation

MUST require more than a single OK dialog before enabling append-only mode on a ledger; SHALL either move the control into a settings view or require a typed confirmation (e.g. the ledger name), because the change restricts editing of the whole ledger.

#### Scenario: Owner enables append-only

- **WHEN** the owner clicks "Enable append-only" on the ledger page
- **THEN** the app asks for explicit typed confirmation (observed: a single `confirm()` dialog that a novice auto-accepts, see `12_after_ledger_create.png` and `templates/ledgers/show.html`).

## Out of Scope
- Full multi-language support (see the existing `localization` spec).
- Login rate-limiting behavior (see the existing `login-rate-limiting` spec).
