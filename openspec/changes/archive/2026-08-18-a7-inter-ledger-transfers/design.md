# ## Context

Without this, users fake it. Fake transfers break reports that sum across
ledgers.

## Goals / Non-Goals

**Goals:**
- Atomic-from-the-user's-perspective movement.

**Non-Goals:**
- Currency revaluation (separate).

## Decisions

- Two `transactions` rows, not one logical one. Each ledger's invariant
  is preserved.
- The link table supports reports that need to eliminate duplicates
  during consolidation.

## Risks / Trade-offs

- Two transactions to roll back if a partial failure occurs. We use a
  Postgres savepoint around both inserts and roll back on error.
