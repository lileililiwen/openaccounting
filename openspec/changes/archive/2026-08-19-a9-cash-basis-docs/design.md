# ## Context

Cash basis is the most-requested small-business feature. The current
read-time filter is correct but does not help users set up the
underlying recognition patterns.

## Goals / Non-Goals

**Goals:**
- Make the existing filter discoverable.
- Provide one-click recognition.

**Non-Goals:**
- Multi-currency deferred revenue (separate change).

## Decisions

- Use the existing `transaction_templates` table to store recognition
  schedules. The templates already support `kind='scheduled'` and a
  recurring cron.
