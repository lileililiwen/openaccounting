# Reports Polish — Tasks

## 1. Testing

- [ ] 1.1 Integration test: the report index links AR aging, AP aging, cash-flow forecast, budget vs actual, tax summary, and amortization.
- [ ] 1.2 Integration test: the index shows "Closed periods: FY2025" after closing a period, and nothing when none closed.
- [ ] 1.3 Integration test: trial balance as-of after closing FY2025 shows the period-closed notice.
- [ ] 1.4 Integration test: income statement covering a closed year shows the notice; one outside it does not.
- [ ] 1.5 Unit test: `closed_notice` formats one year ("FY2025 is closed") and multiple years ("FY2024 and FY2025 are closed").

## 2. Implementation

- [ ] 2.1 Add `closed_years` + `closed_notice` helpers (unit-tested) to `src/handlers/reports.rs`.
- [ ] 2.2 Add `closed_notice: String` to the eight report page structs (index, trial balance, balance sheet, income statement, cash flow, general ledger, aging, cash-flow forecast).
- [ ] 2.3 Compute + pass the notice in the corresponding handlers.
- [ ] 2.4 Add `templates/partials/_period_closed.html` and render it on each report page.
- [ ] 2.5 Add the six missing report cards to `templates/reports/index.html` and a closed-periods note.

## 3. Documentation

- [ ] 3.1 Note report discoverability / period-close notices in the README.
