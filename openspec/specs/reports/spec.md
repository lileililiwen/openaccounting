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

