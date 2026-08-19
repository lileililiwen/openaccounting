# Document and Extend Cash-Basis Support

## Why

The cash-basis toggle (`migrations/0020_add_ledger_basis.sql`) is a
read-time filter on reports, not a posting-time choice. Users running a
cash-basis ledger cannot book deferred revenue cleanly: a Jan invoice
received for $1200 covering a year posts the full $1200 in Jan, which is
incorrect under cash basis AND incorrect under accrual.

## What Changes

- Document the current behavior in `docs/cash-basis.md` and link it
  from the README.
- Add support for `DeferredRevenue` and `PrepaidExpense` account subtypes
  (already exist as `CurrentLiability` and `CurrentAsset`; just document
  the convention).
- Add a `RecognizeRevenue` template action that moves DeferredRevenue →
  Revenue on the chosen date.
- Add a `RecognizeExpense` template action for prepaid expenses.

## Capabilities

### New Capabilities

- `cash-basis-docs`: Cash-basis workflow documentation and template actions.

## Impact

**New files:**
- `docs/cash-basis.md`.
- `src/handlers/templates_recognize.rs`.
- `tests/http/cash_basis_recognize.rs`.

**Modified files:**
- `README.md` — link to docs/cash-basis.md.
- `openspec/specs/reports/spec.md` — clarify basis semantics.
