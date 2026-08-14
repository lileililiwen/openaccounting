# Add Cash-Flow Forecasting

## Why

Cash-flow reports show what happened. **Forecasting** shows what
will happen. hledger is the only OSS tool with first-class
forecasting (`--forecast` flag); no web app does this well.
For a micro-business, the most common failure mode is not
"what did I spend" but "will I have enough in the bank on the
25th to make payroll?" — a forecast is the answer.

This change adds a forward-looking cash projection by
combining the current cash balance with all scheduled (future)
recurring transactions and known scheduled bills, out to a
configurable horizon.

## What Changes

- New routes:
  - `GET /ledgers/{id}/reports/cash-flow-forecast?days=N`
    with N defaulting to 90.
- A new report module `src/reports/cash_flow_forecast.rs` that:
  - Takes today's cash balance (sum across `cash` and `bank`
    accounts).
  - Generates a synthetic transaction list for the next N days
    by materializing all active `recurring_transactions` whose
    `next_run_date` is within `[today, today+N]`.
  - Walks day-by-day, applying each synthetic transaction to
    the running balance.
  - Renders the result as a stacked SVG line chart
    ("projected balance") + a table of upcoming entries.
- New nav link: "Cash flow → Forecast".
- The forecast uses only recurring transactions; manual future
  postings are out of scope.

## Capabilities

### Modified Capabilities

- `reports` — adds the `Cash Flow Forecast` requirement.

## Impact

- **New files:**
  - `src/reports/cash_flow_forecast.rs`
  - `src/handlers/cash_flow_forecast.rs`
  - `src/templates/cash_flow_forecast.rs`
  - `templates/reports/cash_flow_forecast.html`
  - `tests/integration/cash_flow_forecast.rs`
- **Modified files:**
  - `src/main.rs` — 1 new route.
  - `src/reports/mod.rs` — `pub mod cash_flow_forecast;`.
  - `templates/partials/_nav.html` — add "Forecast" link.

## Non-Goals

- Probabilistic / Monte Carlo forecasting. Deterministic only.
- AR/AP aging as forecast input. (Manual future-dated
  transactions not supported.)
- AI-assisted suggestions ("you might run short on day 23").
