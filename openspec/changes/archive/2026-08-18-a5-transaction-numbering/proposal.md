# Per-Ledger Per-Year Transaction Numbers

## Why

Transactions are keyed by UUID; no human reference. Accountants and
auditors expect to cite transactions by number (Invoice #1023, Check
#4001). GnuCash, Akaunting, Firefly III all auto-number per ledger and
period.

## What Changes

- New `transactions.number TEXT` column.
- Auto-generated on insert: `{YYYY}-{NNNNNN}` where `NNNNNN` is the
  per-ledger-per-year zero-padded sequence.
- Sequence maintained by a small `ledger_counters(ledger_id, period,
  count)` table OR by a `SELECT count(*)+1` query inside the same tx.
- User can override the number manually on create.
- Numbers MUST be unique per (ledger_id, year).

## Capabilities

### New Capabilities

- `transaction-numbering`: Auto-numbered transactions.

## Impact

**New files:**
- `migrations/0033_add_transaction_number.sql`.
- `tests/http/transaction_number.rs`.
