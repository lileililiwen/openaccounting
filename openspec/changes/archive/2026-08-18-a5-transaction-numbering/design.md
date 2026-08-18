# ## Context

UUIDs are good for keys, bad for citations.

## Goals / Non-Goals

**Goals:**
- Predictable, year-zero-padded numbering.

**Non-Goals:**
- Continuous (non-resetting) numbering across years.

## Decisions

- `SELECT count(*)+1` inside the same tx is enough for low write rates.
  For higher throughput, switch to a `ledger_counters` table with a row
  lock; not needed at v1.
- The number is shown in the URL `…/transactions/2025-000123` in addition
  to the UUID.

## Risks / Trade-offs

- Race on `count(*)+1` is theoretically possible. Mitigate with the
  UNIQUE index and a retry-on-conflict path.
