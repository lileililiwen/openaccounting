# compliance-exports Specification

## Purpose
TBD - created by archiving change compliance-exports. Update Purpose after archive.
## Requirements
### Requirement: Archivable PDFs

Core reports and invoices SHALL offer server-generated PDF downloads that are byte-stable for the same ledger snapshot and include ledger name, period, generation timestamp, and generating version.

#### Scenario: Same snapshot same bytes

- **WHEN** the fixture ledger's balance sheet PDF is generated twice without data changes
- **THEN** both files are byte-identical.

### Requirement: Machine Exports

The export index SHALL offer SAF-T lite XML, XBRL-GL instance, and DATEV-compatible CSV covering accounts and GL entries for a chosen period, each validated against a checked-in schema or header fixture.

#### Scenario: DATEV CSV opens with expected headers

- **WHEN** a DATEV export is requested for 2026-Q1
- **THEN** the CSV header matches the checked-in fixture and every data row has balanced debit/credit columns.

### Requirement: Comparatives and Drill-Down

P&L and balance sheet SHALL show prior-period columns computed by the same queries with shifted dates. Every amount line SHALL link to the filtered general ledger for that account and period.

#### Scenario: Prior column agrees with re-run

- **WHEN** 2026-Q1 P&L shows 2025-Q1 comparatives
- **THEN** the comparative column equals a direct 2025-Q1 report run.

#### Scenario: Drill-down filters correctly

- **WHEN** a user clicks a P&L expense line
- **THEN** the GL opens pre-filtered to that account and period.

### Requirement: Report Notes

Ledgers SHALL store one notes record per period with author and timestamp, rendered on P&L/BS print and PDF outputs.

#### Scenario: Notes appear on PDF

- **WHEN** notes are saved for 2026-Q1 and the P&L PDF is generated
- **THEN** the notes text and author appear in the PDF footer section.

