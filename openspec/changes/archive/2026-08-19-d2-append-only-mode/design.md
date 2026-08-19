# ## Context

Some industries (US non-profits with restricted funds, regulated trusts)
require non-destructive ledgers.

## Goals / Non-Goals

**Goals:**
- Optional lock.

**Non-Goals:**
- Cryptographic enforcement (D1 covers audit; the append-only toggle is
  application-layer).

## Decisions

- One column; one check in PostingService::create / edit / delete.
