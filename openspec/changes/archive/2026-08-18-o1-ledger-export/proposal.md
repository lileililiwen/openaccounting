# Full Ledger Export (JSON + Beancount Text)

## Why

Only per-report CSV exports exist (`src/lib.rs:268-271`). There is no
"dump everything" endpoint. Hledger's killer feature is
`hledger export → pipe to git → blame → diff`. Beancount is plain text by
default. Without a full export, OpenAccounting data is hard to migrate,
audit, or back up outside Postgres.

## What Changes

- New route `GET /ledgers/{id}/export.json` returns one JSON document
  with every account, transaction, posting, tag, and document reference.
- New route `GET /ledgers/{id}/export.beancount` returns Beancount
  syntax so the same data can be loaded by Beancount / Fava.
- New route `GET /account/export-all.json` returns every ledger the user
  owns or shares.
- Imports of both formats are out of scope for this change (separate
  change).

## Capabilities

### New Capabilities

- `ledger-export`: JSON + Beancount full-ledger export.

## Impact

**New files:**
- `src/export/mod.rs`, `src/export/json.rs`, `src/export/beancount.rs`.
- `src/handlers/export.rs`.
- `tests/http/export.rs`.
