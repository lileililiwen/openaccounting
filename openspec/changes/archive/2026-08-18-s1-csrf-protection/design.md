# ## Context

The current `SessionManagerLayer` (`src/lib.rs:117-121`) only relies on
`SameSite=Lax` cookies. The auth spec (`openspec/specs/auth/spec.md:80`) notes
that production MUST set `Secure=true`, but there is no CSRF token in any form
template or handler. All routes under `protected` (`src/lib.rs:179-547`) are
vulnerable to same-site CSRF as long as a victim visits an attacker page on a
cookie-bearing tab.

## Goals / Non-Goals

**Goals:**
- Synchronizer token bound to the session id, stored only in the session
  payload (never the cookie, never the URL).
- Zero JS required for the round-trip — the hidden field is enough; the HTMX
  meta-tag path is a convenience for SPA-ish flows.
- Exemptions limited to routes that already have stronger auth (provider
  signature) or have no session yet.

**Non-Goals:**
- Double-submit cookie pattern (more fragile, no server-side store).
- Origin/Referer checking alone (insufficient — many corporate proxies strip
  Referer; Origin is missing on some legacy browsers).

## Decisions

- **Where the secret lives.** Inside the `tower-sessions` payload. The
  `CsrfToken` is a 32-byte random value, hex-encoded. Issue lazily on first
  page render, rotate on every successful login.
- **HMAC for binding.** Issue `HMAC-SHA256(session_id, secret)` as the
  per-request token. Verification recomputes the HMAC and constant-time
  compares. This makes the token unforgeable without reading the session id
  even if the token leaks in a log.
- **Layer position.** `CsrfLayer` wraps `protected` *inside* the auth layer so
  anonymous requests are redirected to `/login` first (no DB lookup needed for
  a token check).
- **Constant-time compare.** `subtle::ConstantTimeEq` via the `subtle` crate.

## Risks / Trade-offs

- **Migration**: every form template needs the hidden field added. The
  `_csrf.html` partial is included in `base.html`; the hidden field is rendered
  automatically via an Askama filter. No per-form edits are required.
- **Browser back button**: a cached form after a token rotation will fail
  with 403. The HTTP layer renders a friendly "session expired" page rather
  than a bare 403.
- **Test cost**: a new `tests/http/csrf.rs` integration file is required
  (~12 cases); see the tasks.
