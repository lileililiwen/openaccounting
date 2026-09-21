## 1. Testing

- [x] 1.1 Unit: DATEV header matches fixture; SAF-T lite validates against checked-in XSD.
- [x] 1.2 Unit: comparative query with shifted dates equals direct prior-period run on fixtures.
- [x] 1.3 Integration: PDF generation on fixture ledger is byte-stable across two runs.
- [x] 1.4 Integration: report notes round-trip with author and timestamp.
- [x] 1.5 HTTP: invoice PDF and report PDF endpoints return application/pdf with metadata headers.
- [x] 1.6 HTTP: export index lists SAF-T, XBRL-GL, DATEV links per period.
- [x] 1.7 E2E: report → drill-down → GL filter → back preserves period context.

## 2. Implementation

- [x] 2.1 Export modules for PDF, SAF-T lite, XBRL-GL, DATEV with schema fixtures.
- [x] 2.2 Handlers: PDF download routes for invoices and core reports.
- [x] 2.3 Reports: comparative columns and drill-down links on P&L and balance sheet.
- [x] 2.4 Notes: per-period notes storage, edit UI, print/PDF rendering.
- [x] 2.5 Factur-X: promote invoice e-invoice path to supported with PDF/A-3 embedding.

## 3. Validation

- [x] 3.1 `openspec validate compliance-exports` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including golden PDF and schema tests.