# totp-2fa Specification (delta)

## ADDED Requirements

### Requirement: TOTP Enrollment

MUST allow a user with a verified password to enroll TOTP by scanning a QR code and confirming a 6-digit code; the TOTP secret MUST be stored encrypted at rest.

#### Scenario: Enrollment succeeds

- **WHEN** the user scans the QR and submits a valid 6-digit code
- **THEN** the secret is persisted encrypted; 10 recovery codes are shown once and stored Argon2id-hashed.

#### Scenario: Enrollment rejects bad code

- **WHEN** the user submits a wrong 6-digit code
- **THEN** the secret is discarded; no row is persisted.

#### Scenario: Secret not leaked in responses

- **WHEN** the user fetches their profile or any other endpoint
- **THEN** the secret, encrypted or otherwise, never appears in any JSON/HTML response.

### Requirement: TOTP Verification on Login

MUST require a valid TOTP code (or unused recovery code) after a correct password is supplied, before the session is created.

#### Scenario: Code required

- **WHEN** a 2FA-enrolled user submits correct password
- **THEN** the response is 200 with the 2FA partial; no session cookie is set.

#### Scenario: Code accepted

- **WHEN** the user submits a correct TOTP code
- **THEN** the session is created; redirect to `next`.

#### Scenario: Code rejected

- **WHEN** the user submits a wrong code 5 times
- **THEN** the account is temporarily locked (S2 lockout applies).

#### Scenario: Replay rejected

- **WHEN** the user submits a code twice
- **THEN** the second attempt fails (`last_used_counter` advances).

### Requirement: Recovery Codes

MUST generate 10 single-use recovery codes on enrollment; each code MUST be Argon2id-hashed at rest; consuming a code MUST invalidate it.

#### Scenario: Code redeemable

- **WHEN** the user enters a valid unused code on the 2FA step
- **THEN** the code is consumed; session is created.

#### Scenario: Code one-shot

- **WHEN** the same code is entered twice
- **THEN** the second attempt fails.

#### Scenario: Code Argon2id-hashed at rest

- **WHEN** an admin queries `recovery_codes`
- **THEN** no plaintext codes are visible.

### Requirement: Disable 2FA

MUST require a current password AND a current TOTP code to disable 2FA.

#### Scenario: Disable with both factors

- **WHEN** the user supplies current password + valid code
- **THEN** 2FA is disabled; recovery codes are purged; the secret is deleted.

#### Scenario: Disable with password only

- **WHEN** the user supplies current password only
- **THEN** the disable request is rejected.

#### Scenario: Disable from disabled account

- **WHEN** the user has 2FA disabled and tries to disable again
- **THEN** the response is a no-op 200.

### Requirement: Backup Codes Regeneration

MUST allow regenerating the recovery codes; doing so MUST invalidate the previous codes.

#### Scenario: Regenerate

- **WHEN** the user supplies current password + valid TOTP code
- **THEN** 10 new codes are shown; old codes no longer work.
