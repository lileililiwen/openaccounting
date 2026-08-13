# closing-entries Specification

## Purpose
Enable year-end close of books: transfer income/expense balances to retained earnings, reset income/expense accounts to zero, and produce correct multi-year financial statements.

## Requirements

### Requirement: Retained Earnings Account

Every ledger MUST have a "Retained Earnings" equity account
(subtype `RETAINED_EARNINGS`). This account accumulates the
net income (or loss) from all prior closed periods. It MUST
appear in the equity section of the balance sheet.

The retained earnings account MUST be created automatically as
part of the default chart of accounts, or MUST be created
automatically when the first year-end close is performed.

#### Scenario: Retained earnings appears on balance sheet

- **WHEN** a user views the balance sheet after at least one
  year-end close has been performed
- **THEN** the Retained Earnings line shows the accumulated
  net income from all prior closed periods.

### Requirement: Year-End Close Process

The system MUST provide a "Close Year" action that:

1. Verifies all transactions in the closing period are balanced
   (trial balance is in balance).
2. Creates closing entries: for each income and expense account
   with a non-zero balance, creates a posting that transfers
   the balance to Retained Earnings.
   - For income accounts (credit balance): debit the income
     account, credit Retained Earnings.
   - For expense accounts (debit balance): credit the expense
     account, debit Retained Earnings.
3. Marks the closed period so that no new transactions can be
   posted to dates within or before the closed period.
4. Records the close event in an audit log.

The close process MUST be idempotent: running it twice for the
same period MUST NOT create duplicate closing entries.

#### Scenario: Year-end close transfers income to retained earnings

- **WHEN** a user closes year 2025 and the ledger has $50,000 in
  income and $35,000 in expenses
- **THEN** closing entries are created that transfer $15,000 net
  income to Retained Earnings, and all income/expense accounts
  have zero balance as of December 31, 2025.

#### Scenario: Close process prevents duplicate close

- **WHEN** a user attempts to close the same year twice
- **THEN** the system rejects the second close with an error
  message "Period already closed" and no new entries are created.

### Requirement: Period Locking After Close

After a period is closed, the system MUST reject any transaction
that would post to a date within or before the closed period.
The rejection MUST return `400 Bad Request` with a message
explaining that the period is closed.

#### Scenario: Transaction to closed period is rejected

- **WHEN** a user creates a transaction with `txn_date` in a
  closed period (e.g., 2025-06-15 when 2025 is closed)
- **THEN** the handler returns `400 Bad Request` with the body
  "Period 2025 is closed. Cannot post transactions to closed periods."

### Requirement: Closing Entries Are Tagged

Closing entries MUST be distinguishable from regular transactions.
The system MUST store a `kind` field on transactions with values
like `standard`, `adjusting`, `closing`, `reversing`. Closing
entries created by the year-end close MUST have `kind = 'closing'`.

Reports MUST be able to filter by transaction kind (e.g., show
only standard entries, exclude closing entries).

#### Scenario: Closing entries are tagged

- **WHEN** the year-end close creates closing entries
- **THEN** each closing transaction has `kind = 'closing'` and
  can be filtered in reports.

### Requirement: Income Statement Respects Closed Periods

The income statement for a closed period MUST show the balances
as they were at the time of closing (which should be zero for
all income/expense accounts, since they were closed to retained
earnings). The income statement for the current open period MUST
show only activity within that period.

#### Scenario: Income statement after close shows zero for closed year

- **WHEN** a user views the income statement for 2025 after
  2025 has been closed
- **THEN** all income and expense lines show zero (they were
  closed to retained earnings), and the net income is $0.

### Requirement: Balance Sheet Reflects Retained Earnings

The balance sheet MUST include Retained Earnings in the equity
section. The Retained Earnings balance MUST equal the sum of
all prior years' net income (or minus net loss). The current
year's net income MUST be shown separately until the year-end
close is performed.

#### Scenario: Balance sheet before close shows current year income

- **WHEN** a user views the balance sheet mid-year (before close)
- **THEN** the equity section shows "Current Year Income" as a
  separate line (dynamically computed) plus "Retained Earnings"
  from prior closed years.

#### Scenario: Balance sheet after close includes retained earnings

- **WHEN** a user views the balance sheet after year-end close
- **THEN** the equity section shows "Retained Earnings" as a
  single line that includes the just-closed year's net income.
