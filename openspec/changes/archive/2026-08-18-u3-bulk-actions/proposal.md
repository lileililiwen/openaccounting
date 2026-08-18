# Bulk Actions on Transaction List

## Why

The list page has no row checkboxes; only single-row actions. Real
ledgers have hundreds of transactions needing the same tag, the same
contact, or to be moved to a draft.

## What Changes

- Row checkboxes on `/ledgers/{id}/transactions`.
- A floating action bar appears with: Add tag, Remove tag, Assign
  contact, Mark as draft, Delete.
- All actions are auditable.
- Delete uses reversing entries (no destructive delete).

## Capabilities

### New Capabilities

- `bulk-actions`: Multi-row tag/contact/draft/delete.

## Impact

**New files:**
- `src/handlers/transactions_bulk.rs`.
- `templates/transactions/_bulk_bar.html`.
- `tests/http/transactions_bulk.rs`.
