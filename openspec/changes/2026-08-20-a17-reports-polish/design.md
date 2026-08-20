# Reports Polish — Design

## Approach

Two independent changes. Discoverability is a template-only change to
the report index. Period-close awareness adds one shared helper and a
precomputed notice string threaded through the report pages.

## Decisions

- **A precomputed `closed_notice: String`, not a `closed_years` list in
  the template.** WHY: overlap logic differs per report (point-in-time
  vs period vs forecast) and Askama's expression support is limited.
  Mitigation: handlers compute the overlap with the report's own dates
  and pass a ready-to-render string; empty string renders nothing.

- **Overlap rules:** point-in-time reports (trial balance, balance
  sheet, aging) show the notice for every closed year `<= as_of`; period
  reports (income statement, cash flow, general ledger) show it for
  closed years intersecting `[from, to]`; the forecast shows it for any
  closed year. WHY: a balance sheet as of a date after a closed year
  still includes that closed year's figures. Mitigation: these rules are
  unit-tested.

- **Index shows the closed periods too.** The report index lists closed
  fiscal years (e.g. "Closed periods: FY2025") so the state is visible
  before opening a report. WHY: cheap and consistent. Mitigation: empty
  when nothing is closed.

- **Investment reports stay out of the index.** WHY: they return raw
  JSON, not HTML. Mitigation: noted in the proposal as a non-goal.

## Data Flow

1. `closed_years(pool, ledger_id)` reads `closed_periods.period_year`.
2. Each report handler computes overlap with its dates and builds the
   notice via `closed_notice(years, predicate)`.
3. The notice string is passed in the page struct and rendered by a
   shared `partials/_period_closed.html` partial.

## Risks / Trade-offs

- Many structs gain a field (8 pages). Mitigation: the field is a
  single `String`, and the partial keeps the markup in one place.
- Closing a year after reports were already viewed could surprise a
  user. Mitigation: the notice states the year is closed and final; the
  period-close feature itself is the trigger.
