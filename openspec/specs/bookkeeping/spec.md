# bookkeeping Specification

## Purpose
TBD - created by archiving change bootstrap-double-entry-bookkeeping-engine. Update Purpose after archive.
## Requirements
### Requirement: Double-Entry Invariant

The system SHALL enforce the **double-entry invariant** — for every
committed transaction, `Σ postings.amount where direction='DEBIT' =
Σ postings.amount where direction='CREDIT'`. The invariant MUST be
enforced in two places:

1. In the application handler (`src/handlers/transactions.rs::create`),
   which validates the input and returns `400 Bad Request` with a
   human-readable message if the legs do not balance.
2. In a Postgres `AFTER INSERT OR UPDATE OR DELETE` trigger on
   `postings` (`check_posting_balance`), which raises an exception
   `Postings do not balance: debits=X, credits=Y` if the invariant
   is violated.

A transaction with only one posting is rejected at the handler level
with `400 Bad Request` ("A transaction needs at least two postings").

#### Scenario: Balanced transaction is accepted

- **WHEN** a user submits a transaction with two postings — `100.00`
  debit to Cash and `100.00` credit to Sales Revenue
- **THEN** the transaction is persisted and the handler redirects to
  the transaction detail page with HTTP 303.

#### Scenario: Unbalanced transaction is rejected

- **WHEN** a user submits a transaction with two postings — `100.00`
  debit to Cash and `99.00` credit to Sales Revenue
- **THEN** the handler returns HTTP 400 with the body
  `Postings do not balance: net is 1.00 (debits must equal credits).`
  No row is written to `transactions` or `postings`.

#### Scenario: Direct DB insert of unbalanced postings is rejected

- **WHEN** a SQL client (bypassing the application) inserts two
  postings under one transaction with debits ≠ credits
- **THEN** the `check_posting_balance` trigger raises an exception
  and the INSERT is rolled back. The `transactions` row is not
  committed.

### Requirement: Chart of Accounts

The system SHALL model accounts as rows in `accounts` with one of five
types: `ASSET`, `LIABILITY`, `EQUITY`, `INCOME`, `EXPENSE`. Each
account SHALL have a unique `(ledger_id, name)` tuple and a 3-letter
`currency` code. On ledger creation, the system SHALL seed a default
chart of accounts (see `src/domain/ledger.rs::default_chart_of_accounts`)
covering the typical small-business or personal-finance use case.

The default seeded accounts are:

- **Assets:** Cash on Hand, Bank Account, Accounts Receivable
- **Liabilities:** Accounts Payable, Credit Card
- **Equity:** Owner's Equity, Opening Balances
- **Income:** Sales Revenue, Other Income
- **Expenses:** Office Supplies, Travel & Meals, Software & SaaS,
  Marketing, Professional Services, Rent, Utilities, Other Expense

#### Scenario: User creates a ledger

- **WHEN** a new ledger is created
- **THEN** the `accounts` table contains exactly the 17 default rows
  above, each with `currency` equal to the ledger's `base_currency`.

#### Scenario: Duplicate account name is rejected

- **WHEN** a user tries to add an account with the same `name` as an
  existing account in the same ledger
- **THEN** the handler returns HTTP 409 with body
  `Account name already exists in this ledger`.

### Requirement: Account Types and Normal Direction

For each account type, the **normal direction** (the side that
increases the account) is:

| Type | Normal | Account increases with |
|---|---|---|
| ASSET | Debit | Debit |
| EXPENSE | Debit | Debit |
| LIABILITY | Credit | Credit |
| EQUITY | Credit | Credit |
| INCOME | Credit | Credit |

Reports and the dashboard SHALL compute balances consistent with this
mapping. A positive balance for a debit-normal account SHALL mean
"net debits exceed credits"; for a credit-normal account it SHALL
mean "net credits exceed debits".

#### Scenario: Cash account balance is computed correctly

- **WHEN** the Bank Account has received `1000.00` of debits and
  `400.00` of credits to date
- **THEN** its displayed balance is `600.00` (debit-normal).

#### Scenario: Accounts Payable balance is computed correctly

- **WHEN** the Accounts Payable account has received `300.00` of
  credits and `100.00` of debits to date
- **THEN** its displayed balance is `200.00` (credit-normal, displayed
  as positive because we owe).

### Requirement: Transaction Form

The "new transaction" page MUST:

- Show a date input defaulted to today.
- Require a non-empty `description`.
- Allow optional `payee` and `reference` fields.
- Render N posting rows (default 2), each with:
  - an account `<select>` populated from the ledger's chart of
    accounts (excluding archived);
  - a `DEBIT` / `CREDIT` `<select>`;
  - a positive `<input type="number" step="0.01" min="0.01">`.
- Provide an "+ Add line" button that appends a new posting row via
  inlined JavaScript (no JS build).
- On submission, validate that at least two postings exist and that
  the absolute value of the sum of signed amounts is `0`.

If validation fails, the form is re-rendered with the user's
input preserved and a clear error message.

#### Scenario: User submits an unbalanced form

- **WHEN** the user submits a transaction with debit `100.00` to Cash
  and credit `99.00` to Sales Revenue
- **THEN** the response is HTTP 200 (form re-render) with the error
  banner "Postings do not balance: net is 1.00 (debits must equal
  credits)."

#### Scenario: User adds a third posting

- **WHEN** the user clicks "+ Add line" on an empty form with 2
  default rows
- **THEN** a third posting row is added with empty inputs and
  indexed `lines[2]`.

### Requirement: Per-Ledger Ownership

Every `ledger`, `account`, `transaction`, and `document` belongs to
exactly one user (the ledger's `owner_id`). All read / write
operations on these resources MUST verify that the authenticated
user is the owner; if not, the handler returns HTTP 403 (or 404 to
avoid leaking existence).

#### Scenario: User A tries to read user B's ledger

- **WHEN** user A requests `GET /ledgers/<B's ledger id>/dashboard`
- **THEN** the response is HTTP 404 (not 403, to avoid leaking
  existence).

### Requirement: Multi-Currency Per Ledger (Limited)

Each ledger has a single `base_currency` (3-letter code). The
`currency` on each transaction and account MUST match the ledger's
base currency. Cross-currency transactions are out of scope for v0.1.

#### Scenario: Transaction currency mismatch

- **WHEN** a user tries to create a transaction with `currency=EUR`
  in a ledger whose `base_currency=USD`
- **THEN** the handler returns HTTP 400. (In v0.1 the form hard-codes
  the ledger's currency, so this is defensive only.)

