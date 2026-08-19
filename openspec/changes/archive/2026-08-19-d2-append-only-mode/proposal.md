# Per-Ledger Append-Only Mode

## Why

Once a transaction is posted, it cannot be edited or deleted — only
reversed (which is just another append). The reverse mechanism already
exists (a2) but is opt-in; a per-ledger toggle makes it mandatory.

## What Changes

- New `ledgers.append_only BOOLEAN` column.
- When true, the API and the HTML layer refuse to edit or delete
  transactions and accounts. Reversals are the only allowed correction.
- The audit log records when the mode is toggled.

## Capabilities

### New Capabilities

- `append-only-mode`: Immutable ledger toggle.

## Impact

**New files:**
- `migrations/0046_add_ledger_append_only.sql`.
- `tests/http/append_only.rs`.
