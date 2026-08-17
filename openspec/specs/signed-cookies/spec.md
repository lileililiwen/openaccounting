# signed-cookies Specification

## Purpose
TBD - created by archiving change s7-signed-cookies. Update Purpose after archive.
## Requirements
### Requirement: Cookie Signature

MUST HMAC-SHA256 the session cookie using a key derived from APP_SECRET; MUST reject any cookie whose signature does not verify.

#### Scenario: Tampered cookie

- **WHEN** an attacker flips a byte in the session id
- **THEN** the middleware returns 401 and removes the session.

#### Scenario: Valid cookie

- **WHEN** the unmodified cookie is replayed
- **THEN** the session is accepted.

### Requirement: Key Rotation

MUST support a list of keys (current + previous) so a rolling APP_SECRET rotation does not invalidate all live sessions.

#### Scenario: Two keys

- **WHEN** APP_SECRET and APP_SECRET_PREVIOUS are set
- **THEN** either signature is accepted; new sessions use APP_SECRET.

