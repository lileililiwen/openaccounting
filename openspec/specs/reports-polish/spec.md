# reports-polish Specification

## Purpose
TBD - created by archiving change 2026-08-20-a17-reports-polish. Update Purpose after archive.
## Requirements
### Requirement: Report discoverability

The report index MUST link every rendered report: AR aging, AP aging, cash-flow forecast, budget vs actual, tax summary, and amortization.

#### Scenario: Index lists all reports

- **WHEN** a user opens the report index
- **THEN** it contains links to all six additional reports alongside the existing five.

### Requirement: Period-close awareness

Report pages SHALL show a "period closed" notice when the report's date range (or point-in-time date) overlaps a closed fiscal year.

#### Scenario: Point-in-time after close

- **WHEN** a user views a trial balance or balance sheet as of a date on or after a closed fiscal year
- **THEN** the page shows a notice that the year is closed and the figures are final.

#### Scenario: Period overlaps close

- **WHEN** a user views an income statement or cash flow whose period intersects a closed fiscal year
- **THEN** the page shows the period-closed notice.

#### Scenario: No overlap

- **WHEN** the report's dates do not touch any closed year
- **THEN** no notice is shown.

### Requirement: Index closed-periods note

The report index SHALL list closed fiscal years when any exist.

#### Scenario: Closed periods listed

- **WHEN** a ledger has closed fiscal years
- **THEN** the report index shows them (e.g. "Closed periods: FY2025").

