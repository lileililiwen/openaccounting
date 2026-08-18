# Inter-Ledger and Inter-Entity Transfers

## Why

The multi-entity capability (`migrations/0018_add_multi_entity.sql`,
854 B) is small. No handler actually moves money between ledgers. Today a
sole proprietor with a Personal ledger and an LLC ledger must hand-write a
receivable/payable pair. Firefly III has piggy-bank-style internal
transfers; Akaunting has explicit inter-company journals.

## What Changes

- New `inter_ledger_transfers` table `(id, from_ledger_id,
  to_ledger_id, from_account_id, to_account_id, amount, currency,
  txn_id, fee_amount)`.
- New route `POST /transfers/inter-ledger` that creates one transaction
  per ledger, linked by `inter_ledger_transfers.id`.
- New report: `/reports/inter-entity` showing eliminations.

## Capabilities

### New Capabilities

- `inter-ledger-transfers`: Atomic cross-ledger money movement.

## Impact

**New files:**
- `migrations/0035_add_inter_ledger_transfers.sql`.
- `src/handlers/transfers.rs`.
- `templates/transfers/new.html`.
- `src/reports/inter_entity.rs`.
- `tests/http/transfers.rs`.
