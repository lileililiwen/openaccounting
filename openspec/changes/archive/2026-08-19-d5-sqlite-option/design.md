# ## Context

Major refactor risk.

## Goals / Non-Goals

**Goals:**
- Optional SQLite.

**Non-Goals:**
- Making SQLite the default.

## Decisions

- Feature-gated behind `db-sqlite`.
- Migration files must be SQLite-compatible from now on (separate review).
- Existing tests run on Postgres; SQLite tests run in a separate CI job.
