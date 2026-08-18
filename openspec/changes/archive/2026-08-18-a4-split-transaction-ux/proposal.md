# Split-Transaction Helper UI

## Why

The current `transactions::create` form (`src/handlers/transactions.rs:172`)
accepts arbitrary legs, but the UX is one-line-at-a-time with no helper.
Non-accountants give up. Firefly III has a "Split" button that
auto-balances the second leg against the first.

## What Changes

- New "Add split" button on the new-transaction form.
- Clicking it expands one row into N rows; the first row carries the
  total; the last empty row is auto-balanced to make the postings net to
  zero.
- Drag-to-reorder rows (HTMX swap).
- Server-side: no schema change; the existing multi-leg flow handles it.

## Capabilities

### New Capabilities

- `split-transaction-ux`: Auto-balancing split composer.

## Impact

**Modified files:**
- `templates/transactions/new.html` — add split button + JS for row
  expansion.
- `static/js/split.js` — minimal JS (no build step).
- `tests/http/transactions_split.rs`.
