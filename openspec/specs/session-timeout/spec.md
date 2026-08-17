# session-timeout Specification

## Purpose
TBD - created by archiving change s6-session-timeout. Update Purpose after archive.
## Requirements
### Requirement: Idle Timeout

MUST expire a session that has not seen activity for 30 minutes.

#### Scenario: Inactive 31 min

- **WHEN** no request for 31 minutes
- **THEN** the next request is redirected to `/login` with flash `Your session expired`.

#### Scenario: Active refresh

- **WHEN** any request within 30 minutes
- **THEN** the session is renewed.

### Requirement: Absolute Timeout

MUST expire any session 12 hours after its creation regardless of activity.

#### Scenario: 12 h elapsed

- **WHEN** session was created 12 h ago with constant activity
- **THEN** the next request is redirected to `/login`.

### Requirement: Admin Override

MUST allow an admin to shorten or extend the limits per user via an env var `SESSION_IDLE_SECONDS` and `SESSION_ABSOLUTE_SECONDS` for dev/test.

#### Scenario: Env override

- **WHEN** SESSION_IDLE_SECONDS=900
- **THEN** idle timeout becomes 15 minutes.

