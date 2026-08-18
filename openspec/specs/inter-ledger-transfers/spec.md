# inter-ledger-transfers Specification

## Purpose
TBD - created by archiving change a7-inter-ledger-transfers. Update Purpose after archive.
## Requirements
### Requirement: Single Action

MUST allow the user to move money from one ledger to another with a single form submit; the system creates one transaction in each ledger and links them.

#### Scenario: Simple transfer

- **WHEN** the user transfers $500 from Checking (Personal) to Checking (LLC)
- **THEN** two balanced transactions are created; the transfer row links them.

### Requirement: Currency Conversion

MUST accept a different currency on each side and store the FX rate used.

#### Scenario: USD → EUR

- **WHEN** the user transfers 100 USD to a EUR ledger at rate 0.92
- **THEN** two transactions are created: one for 100 USD, one for 92 EUR; the rate is stored on the transfer row.

### Requirement: Fee

MUST allow a transfer fee on the source side.

#### Scenario: With fee

- **WHEN** the user transfers 100 with a 3 fee from source
- **THEN** source-side Dr Cash 97 / Cr Cash 100, with the 3 going to a Fees expense account.

### Requirement: Authorization

MUST require owner OR editor on BOTH ledgers.

#### Scenario: Editor on one side

- **WHEN** a user who is editor on Personal but only viewer on LLC tries to transfer
- **THEN** 403.

### Requirement: Reversal

MUST allow reversing an inter-ledger transfer via the standard reversing-entry flow.

#### Scenario: Reverse

- **WHEN** the user reverses a transfer
- **THEN** two negation transactions are created; the transfer row is marked reversed.

