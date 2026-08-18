# csv-import-wizard Specification (delta)

## ADDED Requirements

### Requirement: Three Steps

MUST guide the user through upload → map → preview → commit; each step is a separate URL but state is kept in the session.

#### Scenario: Wizard

- **WHEN** the user uploads a CSV
- **THEN** step 2 (map) is shown.

### Requirement: Column Auto-Detection

MUST auto-detect the column for date, amount, description by header name; user can override.

#### Scenario: Auto-detect

- **WHEN** the CSV has a column 'Date'
- **THEN** the date field is pre-selected; the user can change it.

### Requirement: Saved Mappings

MUST allow saving a mapping per (filename-pattern, format) so subsequent uploads skip step 2.

#### Scenario: Saved

- **WHEN** the user saves the mapping as 'bank1'
- **THEN** next upload of a file matching `bank1*.csv` skips step 2.

### Requirement: Preview

MUST show 5 rows transformed before commit; the user can correct individual rows.

#### Scenario: Preview

- **WHEN** the user reaches step 3
- **THEN** the 5 rows are visible with parsed date/amount/description.

### Requirement: Commit

MUST atomically insert all rows; on failure, no rows are inserted.

#### Scenario: Atomic

- **WHEN** the user commits
- **THEN** all rows are inserted in one DB transaction.
