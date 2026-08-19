# append-only-mode Specification (delta)

## ADDED Requirements

### Requirement: Toggle

MUST allow the owner to toggle append-only on a ledger; the audit log records the toggle.

#### Scenario: Toggle on

- **WHEN** the owner enables append-only
- **THEN** an audit row is written with the previous value.

### Requirement: Edit Blocked

MUST refuse any edit or destructive delete on a transaction in an append-only ledger; the response is 422 with a clear message; reversal is offered as the alternative.

#### Scenario: Edit attempt

- **WHEN** an editor tries to edit
- **THEN** 422.

### Requirement: Reversal Allowed

MUST allow reversal even in append-only mode.

#### Scenario: Reverse

- **WHEN** the user reverses
- **THEN** a reversal transaction is created.

### Requirement: Account Blocked

MUST refuse edits to the chart of accounts in append-only mode.

#### Scenario: Rename account

- **WHEN** an editor renames an account
- **THEN** 422.

### Requirement: Cannot Disable by Non-Owner

MUST refuse to disable append-only from anyone other than the owner.

#### Scenario: Editor disables

- **WHEN** an editor tries to disable
- **THEN** 403.
