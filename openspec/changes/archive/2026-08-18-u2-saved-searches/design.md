# ## Context

No saved filters.

## Goals / Non-Goals

**Goals:**
- One-click repeat.

**Non-Goals:**
- Cross-ledger saved searches.

## Decisions

- Stored as `(user_id, ledger_id, name, query_string)`.
- The existing query string is the source of truth; no schema for
  individual filter fields.
