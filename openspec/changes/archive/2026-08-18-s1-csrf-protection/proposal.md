# Add CSRF Protection to All State-Changing Forms

## Why

All protected routes accept form-encoded POST submissions (`src/lib.rs:179-547`).
`SessionManagerLayer` is configured with `SameSite=Lax` (`src/lib.rs:120`), which
mitigates most CSRF vectors but does not eliminate them: top-level cross-site
GET-style form tricks, same-site subdomain attacks, and certain browser quirks
all bypass Lax. OWASP, Firefly III (Laravel CSRF middleware), and Akaunting all
enforce an explicit synchronizer token. OpenAccounting currently does not, so a
crafted page can submit `POST /ledgers/{id}/transactions` (or worse,
`/admin/backups/create`, `/account/password`, `/devices/register`) while the
user is logged in.

## What Changes

- Add a per-session CSRF secret stored in the `tower-sessions` payload.
- Add a `CsrfLayer` middleware that issues a token on every GET response
  (HTML pages only) and verifies the `csrf_token` hidden field on every POST.
- Inject the token via a new `templates/partials/_csrf.html` partial included
  in every form template (and into HTMX form posts via a meta tag fallback).
- Public endpoints (`/login`, `/register`, `/static/*`, the bank-feed webhook)
  are exempt; the webhook has its own provider-signature check.
- All existing tests must still pass; new HTTP tests assert token presence on
  forms, rejection of stale tokens, and round-trip on legitimate POSTs.

## Capabilities

### New Capabilities

- `csrf-protection`: Synchronizer-token CSRF defense for state-changing endpoints.

## Impact

**New files:**
- `src/auth/csrf.rs` (token issue + verify).
- `templates/partials/_csrf.html`.
- `tests/http/csrf.rs`.

**Modified files:**
- `src/lib.rs` — wrap `protected` router with `CsrfLayer`.
- `templates/base.html` — include `_csrf.html`.
- `src/handlers/*.rs` — add hidden field to every form template (HTMX `hx-headers`
  fallback documented in the design).
