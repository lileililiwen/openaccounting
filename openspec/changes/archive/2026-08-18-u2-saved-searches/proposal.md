# Saved Searches / Smart Views

## Why

Filters on `/ledgers/{id}/transactions` are query-string only.
Re-applying the same filter daily is tedious. Firefly III has saved
filters; Akaunting has saved searches.

## What Changes

- New `saved_searches` table.
- New route `/ledgers/{id}/searches` lists saved searches.
- Each search has a name, a query string, and an optional color tag.
- Click → renders the list with the saved query pre-applied.
- A `Make default` button uses it as the landing filter.

## Capabilities

### New Capabilities

- `saved-searches`: Named, reusable transaction filters.

## Impact

**New files:**
- `migrations/0040_add_saved_searches.sql`.
- `src/handlers/saved_searches.rs`.
- `templates/transactions/_searches.html`.
