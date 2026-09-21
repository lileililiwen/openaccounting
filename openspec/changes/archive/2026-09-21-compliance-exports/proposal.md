# Proposal: Archivable reports and compliance exports

## Why

Reports render as HTML with browser-print (`printable-views`) and export to CSV/JSON/Beancount/hledger. There is no deterministic server-side PDF for archiving invoices or statements, no SAF-T/XBRL-GL/DATEV interchange, and reports lack comparative periods, drill-down, and disclosure notes. Factur-X is experimental. Accountants and auditors need archivable, comparable, exportable financials.

## What Changes

- Server-rendered PDF/A outputs for invoices and core reports (trial balance, P&L, balance sheet, GL) with embedded metadata.
- SAF-T lite, XBRL-GL, and DATEV-compatible CSV exports for the ledger.
- Comparative periods (current vs prior) plus click-through drill-down from report lines to ledger entries.
- Report notes field stored per ledger per period for disclosures.
- Factur-X path promoted from experimental to supported for the invoice PDF.

## Capabilities

### New Capabilities
- `compliance-exports`: PDF/A archiving, SAF-T/XBRL-GL/DATEV exports, comparatives, drill-down, report notes.

### Modified Capabilities
- `reports`: report pages gain prior-period columns and drill-down links without changing report math.
- `ledger-export`: export index gains the new machine-readable formats.
- `e-invoicing-facturx`: invoice PDF path becomes supported (was experimental).

## Impact

Affected: `src/reports/*`, `src/handlers/export.rs`, invoice print handlers, export templates, new `src/export/{pdf,saf-t,xbrl,datev}.rs`. Unaffected: posting invariant, auth, bank feeds, scheduler.
