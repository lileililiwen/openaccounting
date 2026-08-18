# pta-export-import Specification (delta)

## ADDED Requirements

### Requirement: Export Subcommand

MUST support `openaccounting export --ledger=<id> --format=beancount` writing the export to stdout.

#### Scenario: Beancount export

- **WHEN** the user runs the command
- **THEN** Beancount text appears on stdout; `bean-check` parses it.

### Requirement: Import Subcommand

MUST support `openaccounting import --ledger=<id> --format=beancount` reading from stdin.

#### Scenario: Beancount import

- **WHEN** the user runs the command
- **THEN** the rows are inserted; reports update.

### Requirement: hledger CSV

MUST support `--format=hledger-csv` for both export and import using the hledger CSV layout.

#### Scenario: hledger round-trip

- **WHEN** the user runs export then import
- **THEN** the imported rows equal the original.

### Requirement: Idempotent Import

MUST detect already-imported transactions by `(date, description, payee, amount)` and skip duplicates.

#### Scenario: Re-import

- **WHEN** the user imports the same file twice
- **THEN** no duplicate rows; the second run is a no-op.

### Requirement: Dry Run

MUST support `--dry-run` that prints the diff without inserting.

#### Scenario: Dry run

- **WHEN** the user supplies --dry-run
- **THEN** no rows are written; the planned diff is shown.
