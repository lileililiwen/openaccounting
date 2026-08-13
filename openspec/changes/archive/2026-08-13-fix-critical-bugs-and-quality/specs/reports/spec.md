# reports Specification

## Purpose
TBD - created by archiving change fix-critical-bugs-and-quality. Update Purpose after archive.

## MODIFIED Requirements

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
