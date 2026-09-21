# statement-reconciliation Specification

## Purpose
TBD - created by archiving change statement-reconciliation. Update Purpose after archive.
## Requirements
### Requirement: Reconciliation Sessions

The system SHALL support statement sessions per bank/cash account with statement close date, statement close balance, opening balance carried from the prior closed session, and status `open | closed`. Opening balance SHALL default to the prior closed session's closing balance.

#### Scenario: Opening balance carries forward

- **WHEN** March session closed at 12,400.00 and April session is created
- **THEN** April opens at 12,400.00 automatically.

### Requirement: Cleared Tracking and Difference Gate

Each session SHALL track cleared lines and compute `difference = stmt_close_balance - (opening + sum(cleared))`. Finishing a session SHALL require difference exactly zero. Closed sessions SHALL lock their lines against unclear.

#### Scenario: Non-zero difference blocks finish

- **WHEN** a session shows difference 25.10 and the user clicks Finish
- **THEN** finish fails with 409 showing the difference and the session stays open.

#### Scenario: Zero difference closes and locks

- **WHEN** difference is 0.00 and the user finishes
- **THEN** status becomes closed and unclear attempts fail with 409.

### Requirement: Unreconcile With Reason

Reopening a closed session SHALL require a reason (min 10 chars) and SHALL write an audit row. Unreconciled lines return to uncleared.

#### Scenario: Reasonless unreconcile rejected

- **WHEN** a user reopens a closed session with an empty reason
- **THEN** the request fails with 400.

### Requirement: CAMT and QBO Import

The statement importer SHALL accept CAMT.053 XML and QBO files alongside existing formats, normalizing to date/amount/payee/reference lines and rejecting unreadable files with filename plus cause.

#### Scenario: CAMT import normalizes lines

- **WHEN** a CAMT.053 fixture with 3 entries is uploaded
- **THEN** 3 candidate lines appear with correct dates and signed amounts.

