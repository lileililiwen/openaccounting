# Add Session Idle and Absolute Timeouts

## Why

Sessions last 30 days (`openspec/specs/auth/spec.md:55`). There is no
inactivity limit; a stolen cookie is valid for a month. OWASP recommends
both an idle timeout (e.g. 30 min) and an absolute timeout (e.g. 8-12 h).

## What Changes

- Track `last_seen_at` per session.
- Sessions expire after 30 minutes of inactivity.
- Sessions expire 12 hours after creation regardless of activity.
- On timeout, the user is redirected to `/login?next=...` with a flash
  `Your session expired`.

## Capabilities

### New Capabilities

- `session-timeout`: Idle + absolute session lifetime limits.

## Impact

**New files:**
- `migrations/0030_add_session_last_seen.sql`.

**Modified files:**
- `src/auth/session.rs` — extend the session payload.
- `src/lib.rs` — middleware that touches `last_seen_at` and enforces
  timeouts.
