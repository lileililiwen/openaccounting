# Centralize All Posting Writes Through PostingService

## Why

The DB trigger `check_posting_balance` (`migrations/0001_init.sql:131-157`)
rejects unbalanced transactions, but it only fires AFTER INSERT or UPDATE on
the `postings` table. The current code paths that create postings include:
`src/handlers/transactions.rs`, `src/handlers/document_ocr.rs`,
`src/handlers/import*.rs`, `src/handlers/reconciliation.rs`,
`src/handlers/reimbursement.rs`, `src/handlers/bank_feeds.rs`,
`src/handlers/closing.rs`. Each one hand-rolls the transaction / posting
insert dance. A future write path that forgets to wrap in a single tx or
to check the period could violate the invariant silently until the trigger
fires — or worse, accept a partial ledger state.

## What Changes

- New `src/domain/posting_service.rs` that owns:
  - Begin tx, insert transaction row, loop postings, validate balance in
  code, commit.
  - Period-close check.
  - Audit-log call.
  - Reversal handling.
- Migrate every handler that currently inserts postings to call
  `PostingService::create(NewTransaction)`.
- The DB trigger remains as a final backstop.

## Capabilities

### New Capabilities

- `posting-service`: Centralized write path for postings.

## Impact

**New files:**
- `src/domain/posting_service.rs`.

**Modified files:**
- All handlers listed in `## Why`.
- `tests/integration/posting_service.rs`.
