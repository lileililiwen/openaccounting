# Amortization Schedules

## Why

OpenAccounting has fixed-asset depreciation (`migrations/0016`) but no
amortization of intangible assets (patents, licenses, software
subscriptions) or deferred revenue. Akaunting and Firefly III both
auto-generate amortization entries.

## What Changes

- New `amortization_schedules` table `(id, source_account_id,
  target_account_id, total_amount, periods, period_unit, start_date,
  end_date)`.
- New route `POST /ledgers/{id}/amortization/new` creates the schedule.
- The existing recurring-transaction worker runs each schedule and posts
  one entry per period.
- Reports show amortization-progress per asset.

## Capabilities

### New Capabilities

- `amortization`: Auto-amortization schedules.

## Impact

**New files:**
- `migrations/0037_add_amortization.sql`.
- `src/handlers/amortization.rs`.
- `src/workers/amortization.rs`.
- `tests/http/amortization.rs`.
