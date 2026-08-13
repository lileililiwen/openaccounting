# period-close Specification

## Purpose
Lock completed accounting periods so historical reports cannot be tampered with. Ensures financial statements are trustworthy after a period is closed.

## Requirements

### Requirement: Period Status Tracking

The system MUST maintain a `periods` table with at minimum:

- `id` UUID PRIMARY KEY
- `ledger_id` UUID NOT NULL
- `period_start` DATE NOT NULL
- `period_end` DATE NOT NULL
- `status` TEXT NOT NULL CHECK (`status` IN ('open', 'closed'))
- `closed_by` UUID (nullable, set when closed)
- `closed_at` TIMESTAMPTZ (nullable, set when closed)

Each period represents a month or fiscal year. Periods MUST NOT
overlap within a ledger.

#### Scenario: Open periods exist for a ledger

- **WHEN** a ledger has transactions in January and February 2026
- **THEN** there are open periods covering those months.

### Requirement: Close Period Action

The system MUST provide a "Close Period" action that:

1. Verifies all transactions within the period are balanced
   (trial balance in balance).
2. Updates the period's status to 'closed'.
3. Records `closed_by` and `closed_at`.
4. Prevents future transactions from posting to dates within
   or before the closed period.

#### Scenario: Period is closed successfully

- **WHEN** a user closes January 2026 and the trial balance is
  in balance
- **THEN** the period status changes to 'closed', and any
  attempt to create a transaction with `txn_date` in January
  returns `400 Bad Request`.

#### Scenario: Close is rejected if trial balance is out of balance

- **WHEN** a user attempts to close a period where debits ≠ credits
- **THEN** the close is rejected with `400 Bad Request` and the
  message "Trial balance is not in balance for this period."

### Requirement: Transaction Date Validation

The transaction creation handler MUST reject any transaction
with a `txn_date` that falls within a closed period. The check
MUST query the `periods` table for the ledger.

#### Scenario: Backdated transaction is rejected

- **WHEN** a user creates a transaction with `txn_date=2026-01-15`
  and January 2026 is closed
- **THEN** the handler returns `400 Bad Request` with the body
  "Period January 2026 is closed."

### Requirement: Period List UI

The system MUST provide a UI page listing all periods for a
ledger with their status (open/closed), date range, and who
closed them (if closed). Open periods MUST have a "Close"
button; closed periods MUST NOT.

#### Scenario: User views period list

- **WHEN** a user navigates to the periods page
- **THEN** they see a table of periods with status indicators
  and can close open periods.
