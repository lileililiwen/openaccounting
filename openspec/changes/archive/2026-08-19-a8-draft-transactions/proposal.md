# Draft Transactions

## Why

`src/handlers/transactions.rs:332` hard-codes `kind = 'standard'`. There
is no way to compose a transaction and save it for later. Accountants
often batch a month's entries; an unfinished entry needs a scratch
area.

## What Changes

- New `kind = 'draft'` and `kind = 'posted'`.
- Drafts skip the period-close check, skip the audit-log write, and are
  excluded from reports.
- A draft can be promoted to posted via a single button.
- A draft can be deleted without creating a reversal entry.

## Capabilities

### New Capabilities

- `draft-transactions`: Draft and post lifecycle.

## Impact

**New files:**
- `migrations/0036_add_draft_kind.sql`.
- `src/handlers/transactions_draft.rs`.
- `tests/http/transactions_draft.rs`.
