# reports Specification

## Purpose
TBD - created by archiving change bootstrap-double-entry-bookkeeping-engine. Update Purpose after archive.
## Requirements
### Requirement: Trial Balance

The trial balance report at
`GET /ledgers/{id}/reports/trial-balance?as_of=YYYY-MM-DD` SHALL list
every account in the ledger that has any non-zero movement on or
before `as_of`, showing:

- `account_name` and `account_type`
- `debit` and `credit` columns, with only the non-zero side populated
  on the **normal** side of the account
- A footer with `totals_debit` and `totals_credit` that MUST be equal
  (the report SHALL display a green "✓ In balance" banner if so, or a
  red "✗ Out of balance — investigate" otherwise).

The default `as_of` is today (UTC).

#### Scenario: Trial balance is balanced for a typical dataset

- **WHEN** the ledger contains 17 default accounts and the user has
  recorded one balanced transaction (debit Cash 100 / credit Sales
  100)
- **THEN** the report shows two rows, the footer totals are
  `100.00` / `100.00`, and the banner is green.

#### Scenario: Filter by `as_of`

- **WHEN** the user requests `?as_of=2025-12-31` and the latest
  transaction is dated `2026-01-15`
- **THEN** the report excludes all postings on or after
  `2026-01-01`.

### Requirement: Balance Sheet

The balance sheet at
`GET /ledgers/{id}/reports/balance-sheet?as_of=YYYY-MM-DD` SHALL
show three sections:

1. **Assets** — every `ASSET` account with its balance as of `as_of`.
2. **Liabilities** — every `LIABILITY` account.
3. **Equity** — every `EQUITY` account, plus a synthetic row "Net
   income (current year)" computed from the income statement from
   Jan 1 to `as_of`.

The report SHALL show `total_assets` and `total_liab_equity` and
display a green banner "✓ Balance sheet balances: A = L + E" iff the
two totals are exactly equal. A red banner is shown otherwise.

#### Scenario: Balanced balance sheet

- **WHEN** the ledger has `total_assets = 1000`, `total_liabilities
  = 400`, `equity = 500`, `net_income = 100`
- **THEN** `total_liab_equity = 1000` and the banner is green.

### Requirement: Income Statement

The income statement at
`GET /ledgers/{id}/reports/income-statement?from=…&to=…` SHALL show:

- All `INCOME` accounts with their credit-normal movement between
  `from` and `to` (inclusive).
- All `EXPENSE` accounts with their debit-normal movement between
  `from` and `to`.
- `total_income`, `total_expense`, and `net_income = total_income -
  total_expense`.

Defaults: `from = January 1 of the current year`, `to = today`.

#### Scenario: Period with activity

- **WHEN** the period `2026-03-01..2026-03-31` contains one Sales
  Revenue credit of `1000.00` and one Software & SaaS debit of
  `120.00`
- **THEN** `total_income = 1000.00`, `total_expense = 120.00`,
  `net_income = 880.00`.

### Requirement: Cash Flow

The cash flow report at
`GET /ledgers/{id}/reports/cash-flow?from=…&to=…` SHALL identify
"cash accounts" by the heuristic:

```sql
type = 'ASSET' AND (LOWER(name) LIKE '%cash%' OR LOWER(name) LIKE '%bank%' OR code LIKE '1%')
```

The report SHALL display:

- `opening` = sum of debit-normal balance across cash accounts on
  the day before `from`.
- `closing` = the same, on `to`.
- `movement = closing - opening`.
- `inflows` = cash accounts whose net movement in the period is
  positive.
- `outflows` = cash accounts whose net movement is negative.
- `total_inflows`, `total_outflows`.

If no cash accounts exist in the ledger, the report shows zeros and
no rows.

#### Scenario: Typical small business

- **WHEN** the ledger has Cash + Bank accounts and the period has
  one inflow to Bank of `500.00` and one outflow from Cash of
  `120.00`
- **THEN** the report shows `inflows = [{Bank, 500.00}]`,
  `outflows = [{Cash, 120.00}]`, `total_inflows = 500.00`,
  `total_outflows = 120.00`, and `movement = opening + 380.00`.

### Requirement: General Ledger

The general ledger at
`GET /ledgers/{id}/reports/general-ledger?from=…&to=…&account_id=…`
SHALL list every posting whose parent transaction's `txn_date` is in
`[from, to]`, optionally filtered to a single account. The page MUST
expose:

- Date (`txn_date`)
- Description (linking to the transaction detail page)
- Payee
- Account name
- Direction (DEBIT or CREDIT, with a colour-coded badge)
- Amount

A "Download CSV" button MUST produce a CSV with the same columns
plus a `memo` column. The CSV is the source of truth for any
external reporting.

The general ledger MUST compute a running balance per account. The
running balance MUST correctly handle the sign convention: Asset and
Expense accounts increase on debit, while Liability, Equity, and
Income accounts increase on credit. The SQL query MUST NOT contain
no-op expressions (e.g. `CASE WHEN ... THEN 0 ELSE 0 END`).

#### Scenario: Filter by account

- **WHEN** `account_id=<Bank's id>` is set
- **THEN** only postings to that account appear in the listing.

#### Scenario: CSV export

- **WHEN** the user clicks "Export CSV"
- **THEN** the response is a `text/csv` file with
  `Content-Disposition: attachment; filename="general_ledger_*.csv"`,
  one header row, and one data row per posting in the period.

#### Scenario: Running balance is computed correctly

- **WHEN** the general ledger report is generated for a ledger
  with transactions
- **THEN** each account's running balance reflects the correct
  sign convention (debit increases for Asset/Expense, credit
  increases for Liability/Equity/Income).

### Requirement: Per-Report Ownership and Date Defaults

Every report MUST be scoped to the authenticated user's ledger. If
the `account_id` filter on the general ledger refers to an account
in a different ledger, the report SHALL show zero rows (no error).

If `from` is greater than `to`, the report SHALL return a 400 with
`from must be <= to`. v0.1 silently swaps the dates.

#### Scenario: Account filter from another ledger

- **WHEN** the user passes an `account_id` that belongs to a
  different ledger they own
- **THEN** the report shows zero rows. No error is raised.

#### Scenario: Inverted date range

- **WHEN** the user requests `?from=2026-12-31&to=2026-01-01` on
  the income statement
- **THEN** the report returns HTTP 400 with the body
  `from must be <= to`.

### Requirement: Per-Ledger Reporting Basis

Each `ledgers` row SHALL have a `basis` column of type
`TEXT NOT NULL DEFAULT 'accrual'` constrained to the enum
`('accrual','cash')`. Migration `0020_add_ledger_basis.sql`
adds the column with this default. Existing ledgers at the time
the migration runs are backfilled to `'accrual'`.

The income-statement and cash-flow report handlers SHALL accept a
`basis` query parameter (`accrual` or `cash`). When the parameter
is absent, the handler uses the ledger's stored `basis` value.

#### Scenario: Default basis is accrual

- **WHEN** the migration runs on an existing database
- **THEN** every row in `ledgers` has `basis='accrual'`.

#### Scenario: Ledger created with cash basis

- **WHEN** the user posts `POST /ledgers/new` with `basis=cash`
- **THEN** the ledger row is created with `basis='cash'` and the
  response is `303 See Other` to the ledger show page.

### Requirement: Cash-Basis Income Statement

When `basis=cash`, the income-statement SQL SHALL exclude every
posting whose peer-leg account has `subtype` in
(`accounts_receivable`, `accounts_payable`). Concretely: only
postings whose peer-leg is a `CASH` or `BANK` (subtype `cash` or
`bank`) account contribute to the totals. All others are
excluded.

When `basis=accrual` (default), behaviour is unchanged.

The report footer SHALL display the active basis in plain text
("Basis: Cash" / "Basis: Accrual") so a user reading the report
cannot mistake which view they are looking at.

#### Scenario: Cash-basis hides unpaid receivable revenue

- **WHEN** the ledger has a transaction "Sale on credit 30 days":
  DR `Accounts Receivable` 1000 / CR `Sales Revenue` 1000, dated
  2026-08-01, and the user runs the income statement for
  `2026-08-01..2026-08-31` with `basis=cash`
- **THEN** the income statement shows zero revenue (the peer-leg
  was AR, not cash) and a footnote "Revenue excluded: 1,000.00
  (AR not yet collected)".

#### Scenario: Accrual basis includes the receivable

- **WHEN** the same scenario is run with `basis=accrual` (or no
  param, on an accrual-default ledger)
- **THEN** the income statement shows `Sales Revenue: 1,000.00`.

### Requirement: Cash-Basis Cash Flow

The cash-flow report SHALL accept the same `basis` parameter.
Because the report already filters to cash accounts, its totals
are identical under both bases in the absence of multi-currency;
the parameter is accepted for symmetry and to record the basis
explicitly in the footer.

#### Scenario: Footer reflects basis

- **WHEN** the user runs `?basis=cash`
- **THEN** the footer reads "Basis: Cash — totals reflect only
  movements through cash and bank accounts.".

### Requirement: UI Basis Selector

The income-statement and cash-flow pages SHALL expose a small
toggle:

```
[ Accrual ]  [ Cash ]
```

Selecting a basis issues a plain GET to the same URL with the
matching `?basis=` query param; no JavaScript is required. The
active option is rendered with a contrasting background.

#### Scenario: Clicking Cash switches the report

- **WHEN** the user is on the accrual income statement and clicks
  the "Cash" button
- **THEN** the browser navigates to
  `…/income-statement?basis=cash` and the rendered totals use the
  cash-basis SQL.

### Requirement: Cash Flow Forecast

The cash-flow-forecast report at
`GET /ledgers/{id}/reports/cash-flow-forecast?days=N` SHALL
project the ledger's cash balance over the next N days
(default 90, maximum 365) by combining:

1. **Today's balance** — sum of `cash` and `bank` subtype
   accounts' balances at the report's start.
2. **Scheduled transactions** — materializing every active
   `recurring_transactions` row whose `next_run_date` falls
   within `[today, today+N]` and whose generated entries are
   not yet posted (i.e. are future-dated).

The report SHALL render:

- A line chart of projected daily balance (SVG, hand-drawn,
  server-rendered — same style as existing charts).
- A table of upcoming entries with columns: `date`, `payee`,
  `description`, `amount`, `category`.
- A footer summary: `min balance`, `min balance date`,
  `max balance`, `max balance date`, `ending balance on day N`.

#### Scenario: Forecast with one recurring bill

- **WHEN** the ledger has cash balance `10,000` today and one
  active recurring transaction `Rent, every 1st of month,
  3,000.00 expense` and the user requests `?days=60` on Aug 14
- **THEN** the projection shows:
  - Aug 14 to Sep 1: balance 10,000
  - Sep 1: balance drops to 7,000
  - Oct 1: balance drops to 4,000
  - Footer: `min=4,000.00 on 2026-10-01`,
    `max=10,000.00 on 2026-08-14`,
    `ending=4,000.00`.

#### Scenario: Forecast with no recurring transactions

- **WHEN** the ledger has no `recurring_transactions`
- **THEN** the chart is flat at today's balance and the table
  is empty.

### Requirement: Forecast Uses Double-Entry Engine

Materialized forecast entries SHALL go through the same
`check_posting_balance` trigger when previewed (the report
shows them as projections, not committed transactions; but a
"Commit all" button on the report SHALL materialize them into
real transactions through the standard handler).

#### Scenario: Commit all writes the entries

- **WHEN** the user clicks "Commit all" on a 30-day forecast
  with 3 generated entries
- **THEN** 3 `transactions` rows are created in the ledger,
  each balanced, and the recurring transactions' `last_run`
  is advanced to the latest generated date.

