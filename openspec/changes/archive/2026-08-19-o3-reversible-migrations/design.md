# ## Context

Irreversible migrations are operationally fragile.

## Goals / Non-Goals

**Goals:**
- Every migration reversible.

**Non-Goals:**
- Reversing data migrations (column drops). Those stay irreversible but
  MUST be guarded.

## Decisions

- Use `-- Reversible:` comment header; sqlx detects it.
- For destructive changes, document why no DOWN exists; the migration is
  one-way on purpose.
