# transaction-numbering Specification

## Purpose
TBD - created by archiving change a5-transaction-numbering. Update Purpose after archive.
## Requirements
### Requirement: Auto-Number

MUST auto-generate a number of the form `{YYYY}-{NNNNNN}` per ledger per year on transaction creation; the counter MUST reset each year.

#### Scenario: First txn of 2025

- **WHEN** the user creates the first txn in ledger L for 2025
- **THEN** the number is `2025-000001`.

#### Scenario: Cross-year

- **WHEN** txn in 2025 and then in 2026
- **THEN** first is `2025-000001`; second is `2026-000001`.

#### Scenario: Cross-ledger

- **WHEN** txn in ledger A and ledger B in the same year
- **THEN** each gets its own counter starting at 1.

### Requirement: Manual Override

MUST allow the user to supply a number on creation; the supplied value MUST be unique per (ledger_id, year).

#### Scenario: Custom number

- **WHEN** the user supplies `2025-EXPENSE-42`
- **THEN** the transaction is stored with that number; subsequent auto-numbers skip it.

### Requirement: Uniqueness

MUST enforce (ledger_id, year, number) uniqueness at the DB level.

#### Scenario: Duplicate

- **WHEN** the user tries to create two txns with the same number
- **THEN** 409.

### Requirement: Searchable

MUST index the number column and expose it on the transaction list and show pages.

#### Scenario: List shows number

- **WHEN** the user visits /transactions
- **THEN** each row shows the number.

### Requirement: Display

MUST display the number on reports (general-ledger, trial balance, balance sheet header) where applicable.

#### Scenario: GL

- **WHEN** the user opens the GL report
- **THEN** the transaction number is in the first column.

