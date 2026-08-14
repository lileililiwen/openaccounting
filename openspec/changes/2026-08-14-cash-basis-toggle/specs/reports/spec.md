# reports Specification (delta)

## ADDED Requirements

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

The cash-flow report accepts the same `basis` parameter. Because
the report already filters to cash accounts, its totals are
identical under both bases in the absence of multi-currency;
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
