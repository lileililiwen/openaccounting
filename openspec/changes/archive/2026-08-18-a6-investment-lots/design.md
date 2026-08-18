# ## Context

No investment tracking today. The double-entry invariant still holds: a
buy is `Dr Investment / Cr Cash`; a sell is `Dr Cash / Cr Investment / Dr
Realized Gain or Loss`. The lot table adds accounting metadata without
violating the ledger.

## Goals / Non-Goals

**Goals:**
- FIFO. Correct realized gains.

**Non-Goals:**
- Automatic market quotes.
- Multi-currency lots (separate change).
- Specific-ID lot selection (broker-supplied IDs).

## Decisions

- Lots are derived from postings, not separate journal entries.
- A nightly worker recomputes realized gains to detect mis-matches.

## Risks / Trade-offs

- Wash-sale rules (US) are out of scope.
- Cost basis becomes complex with corporate actions (splits, dividends) —
  out of scope for v1.
