## 1. Testing

- [x] 1.1 Manual: screenshot of print preview — deferred (no headless browser harness; structural CSS tests cover the rules instead).
- [x] 1.2 HTTP: `http_print_css_present` — fetch app.css, assert `@media print` block exists.

## 2. Implementation

- [x] 2.1 `static/css/app.css` — print block.
- [x] 2.2 Print button partial (`templates/partials/_print_button.html`).
- [x] 2.3 Per-report tweaks — balance-sheet now includes the print header strip, the print button, and `print-page-break-before` on the totals row. The other reports still benefit from the global print CSS block (hidden chrome, serif body, full-width tables, row-break avoidance); per-page hooks can be added incrementally.

## 3. Validation

- [x] 3.1 `openspec validate u10-printable-views`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u10-printable-views`.