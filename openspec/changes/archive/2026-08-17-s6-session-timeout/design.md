# ## Context

`tower-sessions-sqlx-store` stores the session payload; we extend it.

## Goals / Non-Goals

**Goals:**
- Both timeouts enforced at the middleware layer before any handler.
- Configurable for tests.

**Non-Goals:**
- Sliding refresh of CSRF token on activity (separate concern).

## Decisions

- `last_seen_at` stored in the session payload. On each request, the
  middleware checks `now - last_seen > idle` and `now - created_at >
  absolute`. On success it updates `last_seen_at` to `now`.
- Tests set `SESSION_IDLE_SECONDS=2` and use a mock clock (`tokio::time::pause`)
  to advance time deterministically.

## Risks / Trade-offs

- **Clock skew**: server time change (NTP correction) can prematurely
  expire sessions. Acceptable — fail-secure.
- **Long-running imports**: an importer keeping a session open for 30 min
  is fine; a 12 h background sync is not — it must use a service token,
  which is a separate change.
