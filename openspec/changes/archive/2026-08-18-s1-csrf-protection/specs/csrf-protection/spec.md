# csrf-protection Specification (delta)

## ADDED Requirements

### Requirement: CSRF Token Issuance

MUST Every authenticated HTML response (`GET /ledgers/*`, `/dashboard`,
         `/account`, `/admin/*`, etc.) MUST contain a `<input type="hidden"
         name="csrf_token" value="...">` field rendered from a session-bound
         secret. The token MUST be bound to the session id and MUST rotate on
         login/logout.

#### Scenario: Token present on form

- **WHEN** an authenticated user renders any page that contains a POST form
- **THEN** the HTML response includes exactly one `csrf_token` hidden input scoped to that session.

#### Scenario: Token rotates on login

- **WHEN** the user logs in
- **THEN** any token issued before the login is invalidated; a fresh token is bound to the new session.

#### Scenario: Token rotates on logout

- **WHEN** the user logs out
- **THEN** any pending token is dropped; reuse after re-login fails.

### Requirement: CSRF Token Verification

MUST Every `POST` handler behind `protected` MUST reject the request when
         the `csrf_token` form field (or `X-CSRF-Token` header for HTMX) is
         missing, malformed, or does not match the session-bound secret. The
         handler MUST return HTTP 403 with a generic message; it MUST NOT
         leak the reason.

#### Scenario: Missing token

- **WHEN** an authenticated POST omits `csrf_token`
- **THEN** the response is 403; no DB write occurs.

#### Scenario: Wrong token

- **WHEN** an authenticated POST supplies a token from a different session
- **THEN** the response is 403.

#### Scenario: Replayed token after logout

- **WHEN** the same token is reused after logout-then-login
- **THEN** the response is 403.

#### Scenario: Legitimate POST

- **WHEN** an authenticated POST supplies the correct token
- **THEN** the request is processed normally.

### Requirement: CSRF Exemptions

MUST The following routes are exempt from CSRF verification because they
         have an equivalent external authentication check:
         - `POST /login` (form has no session yet)
         - `POST /register` (form has no session yet)
         - `POST /ledgers/{id}/webhooks/plaid` (provider signature)
         - `GET /static/*` (read-only)
         - `GET /login`, `GET /register` (render the form that issues the token)

#### Scenario: Webhook exempt

- **WHEN** Plaid POSTs a webhook without a CSRF token
- **THEN** the webhook signature is verified and the request is processed.

### Requirement: HTMX Compatibility

MUST HTMX forms MUST work without explicit per-form token fields. A
         `<meta name="csrf-token" content="...">` tag is rendered into
         `templates/base.html`, and an HTMX config script reads the meta and
         sets `htmx:configRequest` to add `X-CSRF-Token` to every request.

#### Scenario: HTMX form post

- **WHEN** an HTMX-driven form submits
- **THEN** the X-CSRF-Token header is set automatically; the request succeeds.
