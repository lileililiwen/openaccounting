# ## Context

Depreciation for fixed assets exists; amortization for intangibles and
deferred items does not.

## Goals / Non-Goals

**Goals:**
- One click → N entries.

**Non-Goals:**
- Variable amortization (step-up, step-down) — separate.

## Decisions

- Reuse the recurring-transaction worker for idempotency.
