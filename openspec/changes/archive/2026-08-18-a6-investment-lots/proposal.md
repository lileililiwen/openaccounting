# Investment Lots and Cost-Basis Tracking

## Why

OpenAccounting has no support for stock or crypto holdings. GnuCash,
hledger, Beancount, Firefly III (via Firefly-Data Importer + extensions)
all support at least basic FIFO cost basis. Without it the system cannot
be the bookkeeping source of truth for a freelancer who trades.

## What Changes

- New `investment_lots` table `(id, account_id, acquired_at, qty,
  unit_cost, currency, source_txn_id)`.
- New `investment_disposals` table `(id, lot_id, qty, unit_proceeds,
  disposed_at, source_txn_id)`.
- FIFO default; LIFO optional later.
- New report: `holdings` (per account, qty, cost basis, market value).
- New report: `realized-gains` (per disposal, gain/loss).

## Capabilities

### New Capabilities

- `investment-lots`: Lot-based cost-basis tracking.

## Impact

**New files:**
- `migrations/0034_add_investment_lots.sql`.
- `src/domain/investment_lot.rs`.
- `src/reports/holdings.rs`, `src/reports/realized_gains.rs`.
- `templates/reports/holdings.html`, `realized-gains.html`.
- `tests/integration/investment_lots.rs`.
