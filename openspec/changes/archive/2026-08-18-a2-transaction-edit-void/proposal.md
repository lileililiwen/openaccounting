# Transaction Edit and Void via Reversing Entries

## Why

`src/lib.rs:198-214` exposes `GET` and `POST` for new transactions but
no edit / void / reverse. A typo means the only recovery is a manual
reversing entry (which has no UI affordance) or calling support. GnuCash,
Firefly III, hledger, Akaunting all allow editing transactions; all record
the edit in the audit log.

## What Changes

- New `kind = 'reversal'` transaction (existing kinds table extended).
- New route `POST /ledgers/{id}/transactions/{txn_id}/reverse` that
  creates a reversal on today's date with negated amounts and a link back
  to the original (`reverses_id` column).
- New route `POST /ledgers/{id}/transactions/{txn_id}/edit` that creates
  a *new* transaction on today's date with the corrected values and
  reverses the original. Net effect: original is preserved, edit is a
  delta.
- Edit/reverse both write audit log entries with the diff JSON.

## Capabilities

### New Capabilities

- `transaction-edit-void`: Reversing-entry based correction.

## Impact

**New files:**
- `migrations/0032_add_reversal_link.sql`.
- `src/handlers/transactions_edit.rs`.
- `templates/transactions/_edit.html`, `_reverse.html`.
- `tests/http/transactions_edit.rs`.

**Modified files:**
- `src/handlers/transactions.rs` — wire new routes.
- `src/lib.rs` — register routes.
