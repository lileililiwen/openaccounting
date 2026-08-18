# transaction-edit-void Specification (delta)

## ADDED Requirements

### Requirement: Reversal Entry

MUST allow reversing any transaction by creating a new dated reversal transaction with all amounts negated and a foreign key to the original; the original MUST NOT be modified or deleted.

#### Scenario: Reverse a posted txn

- **WHEN** the user POSTs to `/transactions/{id}/reverse`
- **THEN** a new transaction is created on today's date with negated postings; the original is unchanged.

#### Scenario: Reports exclude reversed

- **WHEN** the trial balance is run after the reversal
- **THEN** the pair nets to zero; both are listed in the general ledger with a `↶` marker.

### Requirement: Edit Entry

MUST allow editing a transaction by creating a new dated transaction with corrected values and simultaneously reversing the original in the same atomic operation.

#### Scenario: Edit a typo

- **WHEN** the user submits a corrected transaction body
- **THEN** the original is reversed and the corrected transaction is created, in one DB transaction.

#### Scenario: Edit blocked for closed period

- **WHEN** the user tries to edit a transaction in a closed fiscal year
- **THEN** the response is 422.

### Requirement: Audit Trail

MUST record the edit/reversal in the audit log with the full diff JSON and the user id; the audit row is immutable.

#### Scenario: Audit row written

- **WHEN** the user reverses a transaction
- **THEN** an `audit` row exists with action=`reverse` and old/new JSON.

### Requirement: No Edit of Reversal

MUST reject any attempt to reverse or edit a transaction that is itself a reversal; the user must reverse the original.

#### Scenario: Reverse a reversal

- **WHEN** the user reverses a `kind='reversal'` transaction
- **THEN** 422 with `Cannot reverse a reversal`.

### Requirement: Authorization

MUST require owner OR editor on the ledger to reverse/edit.

#### Scenario: Viewer reverses

- **WHEN** a viewer tries to reverse
- **THEN** 403.
