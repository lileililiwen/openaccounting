# Empty States with Illustrations

## Why

Likely: empty lists show "No data" with no guidance. First-time users
don't know what to click next.

## What Changes

- New `_empty.html` partial per resource (transactions, accounts,
  invoices, budgets, reports).
- Each empty state has an illustration (SVG, hand-drawn, no JS) and a
  primary CTA.
- Tests assert that the empty partial is rendered when no rows exist.

## Capabilities

### New Capabilities

- `empty-states`: Polished empty states.

## Impact

**New files:**
- `templates/partials/_empty.html` (template).
- `static/img/empty/{transactions,accounts,invoices,budgets}.svg`.
