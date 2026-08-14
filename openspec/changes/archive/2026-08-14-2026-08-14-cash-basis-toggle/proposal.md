# Add Cash-Basis Report Toggle

## Why

The README claims the system supports **cash-basis and
double-entry** accounting. The current code only supports the
latter: every report queries `postings` joined to `transactions`
and sums movement regardless of the cash event. For the lower
half of the stated audience — individuals and very small
businesses — cash-basis is the natural model and the absence of
the toggle excludes them.

This change does not alter the underlying double-entry engine.
It adds a single optional query parameter (`basis=cash|accrual`)
on the income-statement and cash-flow reports, with cash-basis
excluding the impact of any posting whose account is an AR or AP
accrual type. The general-ledger and trial-balance reports
remain accrual (those reports are inherently cash-neutral — they
show movements, not net income).

## What Changes

- Fix the README's claim that "the system supports cash-basis
  and double-entry": today's code only honors double-entry.
  Rewrite the relevant paragraph to describe the toggle that
  this change introduces, not a feature that already exists.
- New `basis` column on `ledgers` (default `accrual`).
- New enum `ReportBasis { Accrual, Cash }` in `src/reports/`.
- `income_statement` and `cash_flow` handlers accept
  `?basis=…` (default = ledger's `basis`).
- Cash-basis variant of the income statement EXCLUDES:
  - All INCOME and EXPENSE postings whose counter-leg is an AR or
    AP account (subtype `accounts_receivable` or
    `accounts_payable`).
  - i.e. revenue is only counted when cash is received; expense
    only when cash is paid.
- The cash-flow report is unchanged in semantics (it already
  filters by cash accounts), but it now accepts the same `basis`
  parameter and is honest about which basis it computes.
- New nav toggle on each report page (UI: "Basis: Accrual / Cash"
  buttons, server-rendered).

## Capabilities

### New Capabilities

(none — covered by modified capability below)

### Modified Capabilities

- `reports` — add `basis=cash|accrual` query parameter to
  income statement and cash flow; add new ledger column.

## Impact

- **New files:**
  - `migrations/0020_add_ledger_basis.sql`
  - `tests/integration/cash_basis.rs`
- **Modified files:**
  - `src/domain/ledger.rs` — `Ledger` struct gains `basis: ReportBasis`.
  - `src/reports/income_statement.rs` — `run(..., basis)`.
  - `src/reports/cash_flow.rs` — `run(..., basis)`.
  - `src/handlers/reports.rs` — parse `basis` query param, pass
    through.
  - `templates/reports/income_statement.html` — add basis
    selector.
  - `templates/reports/cash_flow.html` — add basis selector.
  - `src/handlers/ledgers.rs` — accept `basis` on `create`,
    default `accrual`.
  - `templates/ledgers/new.html` — add the basis selector to
    the form.
  - `README.md` — rewrite the cash-basis paragraph so it
    describes the toggle rather than an already-shipped feature.
  - `openspec/specs/reports/spec.md` — new requirement.

## Non-Goals

- Per-user basis preference (one basis per ledger).
- Mid-year basis changes (the column can be flipped any time, but
  no historical snapshotting).
- True cash-basis transaction model (i.e. cash-basis without
  double-entry). The system stays double-entry internally; the
  toggle only filters the report query.
- Multi-currency cash-basis (already out of scope per existing
  spec).
