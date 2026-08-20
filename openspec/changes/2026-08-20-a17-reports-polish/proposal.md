# Reports Polish

## Why

Two problems with the reporting surface:

1. **Undiscoverable reports.** The report index links only five reports
   (trial balance, balance sheet, income statement, cash flow, general
   ledger). AR/AP aging, cash-flow forecast, budget vs actual, tax
   summary, and amortization all exist as routes but a user has no way
   to find them.
2. **No period-close awareness.** Reports are computed over all data,
   including closed periods, but nothing tells the user that a fiscal
   year has been closed — i.e. that those numbers are final. A user who
   closed FY2025 needs to know that FY2025 figures are locked.

## What Changes

- The **report index** links every rendered report: AR aging, AP aging,
  cash-flow forecast, budget vs actual, tax summary, and amortization.
- The report pages show a **"period closed" notice** when the report's
  date range (or point-in-time) overlaps a closed fiscal year — e.g.
  "FY2025 is closed — these figures are final."

## Capabilities

- `report-discoverability`: every rendered report is linked from the
  report index.
- `period-close-awareness`: report pages surface closed fiscal years.

## Non-Goals

- Blocking or altering report numbers for closed periods (closing
  entries are already posted as transactions; the numbers are correct).
- Adding investment reports (holdings / realized gains / inter-entity)
  to the index — they render raw JSON, not HTML.
- Multi-year closing (the app closes by calendar year).
