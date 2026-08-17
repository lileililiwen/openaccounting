# password-strength Specification

## Purpose
TBD - created by archiving change s4-password-strength. Update Purpose after archive.
## Requirements
### Requirement: Minimum Length

MUST reject any password shorter than 12 characters at registration and at password change.

#### Scenario: 11 chars

- **WHEN** the user supplies an 11-character password
- **THEN** the response is 400 with `Password must be at least 12 characters`.

#### Scenario: 12 chars

- **WHEN** the user supplies a 12-character password not in the breach list
- **THEN** the password is accepted.

### Requirement: Common Password Deny List

MUST reject any password that appears in the bundled top-100k common-password list.

#### Scenario: Password123

- **WHEN** the user supplies `Password123`
- **THEN** the response is 400 with `This password is too common`.

#### Scenario: Random 16 chars

- **WHEN** the user supplies a random 16-char string
- **THEN** the password is accepted.

### Requirement: Optional Online Breach Check

MUST allow optional integration with the HIBP Pwned Passwords range API using k-anonymity when the `hibp-online` feature is enabled; the full password hash MUST NOT leave the server.

#### Scenario: HIBP feature disabled

- **WHEN** the build is compiled without the feature
- **THEN** only the offline deny list is consulted.

#### Scenario: HIBP feature enabled, breached

- **WHEN** the user supplies a known-breached password
- **THEN** the password is rejected.

#### Scenario: HIBP unreachable

- **WHEN** the HIBP API returns 5xx
- **THEN** the request is allowed (fail-open) and a warning is logged.

### Requirement: No Password Echo

MUST never include the supplied password in any log, error message, or HTTP response body.

#### Scenario: Log scrub

- **WHEN** an attacker triggers an error that includes the form body
- **THEN** the password field is replaced by `***` before logging.

