# reports Specification (delta)

## ADDED Requirements

### Requirement: Cash Flow Forecast

The cash-flow-forecast report at
`GET /ledgers/{id}/reports/cash-flow-forecast?days=N` SHALL
project the ledger's cash balance over the next N days
(default 90, maximum 365) by combining:

1. **Today's balance** — sum of `cash` and `bank` subtype
   accounts' balances at the report's start.
2. **Scheduled transactions** — materializing every active
   `recurring_transactions` row whose `next_run_date` falls
   within `[today, today+N]` and whose generated entries are
   not yet posted (i.e. are future-dated).

The report SHALL render:

- A line chart of projected daily balance (SVG, hand-drawn,
  server-rendered — same style as existing charts).
- A table of upcoming entries with columns: `date`, `payee`,
  `description`, `amount`, `category`.
- A footer summary: `min balance`, `min balance date`,
  `max balance`, `max balance date`, `ending balance on day N`.

#### Scenario: Forecast with one recurring bill

- **WHEN** the ledger has cash balance `10,000` today and one
  active recurring transaction `Rent, every 1st of month,
  3,000.00 expense` and the user requests `?days=60` on Aug 14
- **THEN** the projection shows:
  - Aug 14 to Sep 1: balance 10,000
  - Sep 1: balance drops to 7,000
  - Oct 1: balance drops to 4,000
  - Footer: `min=4,000.00 on 2026-10-01`,
    `max=10,000.00 on 2026-08-14`,
    `ending=4,000.00`.

#### Scenario: Forecast with no recurring transactions

- **WHEN** the ledger has no `recurring_transactions`
- **THEN** the chart is flat at today's balance and the table
  is empty.

### Requirement: Forecast Uses Double-Entry Engine

Materialized forecast entries SHALL go through the same
`check_posting_balance` trigger when previewed (the report
shows them as projections, not committed transactions; but a
"Commit all" button on the report SHALL materialize them into
real transactions through the standard handler).

#### Scenario: Commit all writes the entries

- **WHEN** the user clicks "Commit all" on a 30-day forecast
  with 3 generated entries
- **THEN** 3 `transactions` rows are created in the ledger,
  each balanced, and the recurring transactions' `last_run`
  is advanced to the latest generated date.
