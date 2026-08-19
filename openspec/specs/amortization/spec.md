# amortization Specification

## Purpose
TBD - created by archiving change a10-amortization. Update Purpose after archive.
## Requirements
### Requirement: Schedule Creation

MUST accept a source account, a target account, a total amount, a period unit (monthly, quarterly, yearly), and a number of periods; the system MUST compute the per-period amount and end date.

#### Scenario: Create schedule

- **WHEN** the user schedules $1200 over 12 months
- **THEN** 12 entries of $100 are queued.

### Requirement: Auto-Posting

MUST auto-post one balanced transaction per period on the period's last day; the worker MUST be idempotent (re-running the worker on the same day MUST NOT post twice).

#### Scenario: Worker posts

- **WHEN** the worker runs on day 1 of month 2
- **THEN** a new transaction is posted with kind=amortization.

#### Scenario: Worker idempotent

- **WHEN** the worker runs again on the same day
- **THEN** no new transaction.

### Requirement: Progress Report

MUST show amortization progress per schedule on a `/reports/amortization` page.

#### Scenario: Progress

- **WHEN** the user opens the page
- **THEN** each schedule is listed with posted / remaining / next date.

### Requirement: Manual Overrides

MUST allow marking a period as skipped (e.g. customer dispute) without breaking the schedule.

#### Scenario: Skip

- **WHEN** the user skips period 3
- **THEN** period 3 is recorded as skipped; the schedule continues from period 4.

### Requirement: Authorization

MUST require owner OR editor on the ledger.

#### Scenario: Viewer creates

- **WHEN** a viewer tries to create a schedule
- **THEN** 403.

