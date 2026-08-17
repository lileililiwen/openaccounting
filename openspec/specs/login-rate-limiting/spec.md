# login-rate-limiting Specification

## Purpose
TBD - created by archiving change s2-login-rate-limiting. Update Purpose after archive.
## Requirements
### Requirement: Failed-Login Counter

MUST record every login attempt (success and failure) with the lowercased email, the client IP, the timestamp, and the outcome.

#### Scenario: Success recorded

- **WHEN** a user logs in successfully
- **THEN** one row is inserted with `success=true`.

#### Scenario: Failure recorded

- **WHEN** a user supplies a wrong password
- **THEN** one row is inserted with `success=false`.

### Requirement: Per-Account Throttle

MUST reject a login for an account that has had 5 failed attempts in the last 10 minutes with HTTP 429 and a generic message; the response MUST NOT reveal whether the email exists.

#### Scenario: 5 failures in 10 min

- **WHEN** the same email has 5 failed attempts in the last 10 min
- **THEN** the next attempt returns 429 with body `Too many attempts`.

#### Scenario: Success resets counter

- **WHEN** a user logs in successfully after 4 failures
- **THEN** the counter is reset to zero.

#### Scenario: Indistinguishable from wrong password

- **WHEN** an attacker tries a known email with a wrong password while throttled
- **THEN** the response is identical to a non-throttled wrong password.

### Requirement: Per-IP Throttle

MUST reject logins from a single IP that has had 20 failed attempts against ANY email in the last hour with HTTP 429.

#### Scenario: 20 failures from one IP

- **WHEN** 20 failed attempts from IP 1.2.3.4 in 60 min across multiple emails
- **THEN** the next attempt from that IP returns 429.

#### Scenario: IP does not affect other users

- **WHEN** user B from IP 1.2.3.4 successfully logs in after the throttle
- **THEN** user A from IP 5.6.7.8 is unaffected.

### Requirement: Lockout Reset

MUST allow the account to attempt again after the cooldown window expires; the row count is windowed, not cumulative.

#### Scenario: Cooldown expires

- **WHEN** 11 minutes after the 5th failure
- **THEN** the user can attempt again.

### Requirement: Audit Retention

MUST delete `login_attempts` rows older than 90 days via a daily worker.

#### Scenario: Prune runs

- **WHEN** the daily worker runs
- **THEN** rows older than 90 days are deleted.

