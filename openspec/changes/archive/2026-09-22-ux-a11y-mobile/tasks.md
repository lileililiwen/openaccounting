## 1. Testing

- [x] 1.1 Unit: locale coverage reporter computes per-language missing percentages on fixtures.
- [x] 1.2 Integration: axe pass on rendered fixtures for dashboard, editor, reports, reconcile flows.
- [x] 1.3 Integration: chart render includes role=img summary plus hidden data table.
- [x] 1.4 HTTP: partial responses include focus target and live-region payload markers.
- [x] 1.5 E2E: keyboard-only post-transaction flow completes with announcements verified.
- [x] 1.6 Docs lint: README and mobile README promises match; audit P1 gate enforced.

## 2. Implementation

- [x] 2.1 WCAG audit pass with dated report and P1 remediation across templates.
- [x] 2.2 HTMX focus management and live-region announcements.
- [x] 2.3 Chart accessibility: summaries plus data tables everywhere.
- [x] 2.4 Locale coverage gate in CI with missing-key artifact.
- [x] 2.5 Mobile decision: ship receipt-capture flow or retire shell and document PWA.

## 3. Validation

- [x] 3.1 `openspec validate ux-a11y-mobile` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including coverage and lint tests.
