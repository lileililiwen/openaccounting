# account-management Specification (delta)

## ADDED Requirements

### Requirement: Account edit

MUST allow a ledger writer to edit an account's name, code, and description at any time, and to change its type/subtype while the account has no postings; edits MUST be recorded in the audit log.

#### Scenario: Rename an account

- **WHEN** a writer edits an account's name and saves
- **THEN** the account row is updated and the audit log records the edit.

#### Scenario: Type locked with history

- **WHEN** an account already has postings and the writer attempts to change its type or subtype
- **THEN** the change is rejected and the form explains that type/subtype are locked for accounts with history.

#### Scenario: Type editable without history

- **WHEN** an account has no postings
- **THEN** the writer can change its type and subtype.

### Requirement: Account archive and activate

MUST allow a ledger writer to archive an account (hiding it from new-entry pickers while keeping it in reports) and to reactivate it.

#### Scenario: Archive hides from pickers

- **WHEN** a writer archives an account with history
- **THEN** the account is flagged archived, no longer appears in the new-transaction picker, but still appears in reports and the chart with an "archived" badge.

#### Scenario: Activate restores

- **WHEN** a writer activates an archived account
- **THEN** it appears in pickers again.

### Requirement: Opening balances

MUST allow a ledger writer to record starting balances for balance-sheet accounts as a single balanced transaction against the ledger's "Opening Balances" equity account, dated on the ledger's start date, and MUST refuse a second opening-balances entry.

#### Scenario: Record opening balances

- **WHEN** a writer enters opening balances and saves
- **THEN** one balanced transaction is created dated on the ledger start, with legs in each account's normal direction and the total contra-posted to the Opening Balances equity account.

#### Scenario: Second entry refused

- **WHEN** a writer attempts to save opening balances after one already exists
- **THEN** the save is refused with a clear message.

#### Scenario: All zero amounts

- **WHEN** every opening-balance amount is zero
- **THEN** no transaction is created.
