## 1. Testing

- [x] 1.1 HTTP: `http_recognize_revenue_creates_monthly_template`.
- [x] 1.2 HTTP: `http_recognize_expense_creates_monthly_template`.
- [x] 1.3 HTTP: `http_recognize_template_fires_via_process_due`.
- [x] 1.4 Docs: `docs_cash_basis_md_exists_and_readme_references_it`.

## 2. Implementation

- [x] 2.1 `docs/cash-basis.md` (read-time basis, recognized accounts, examples).
- [x] 2.2 `src/handlers/templates_recognize.rs` (revenue + expense handlers, schema validation).
- [x] 2.3 README link to `docs/cash-basis.md`.
- [x] 2.4 `openspec/specs/reports/spec.md` clarifies basis semantics.
- [x] 2.5 Wired `templates_recognize::router()` into `src/lib.rs`.
- [x] 2.6 Wired `templates::process_due` route for the recurring worker.
- [x] 2.7 Pre-existing bugfix: `run_template` now sets `created_by` and uses `postings` columns that exist.
- [x] 2.8 Pre-existing bugfix: `transactions_kind_check` widened to allow `recurring`.

## 3. Validation

- [x] 3.1 `openspec validate a9-cash-basis-docs`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support --test integration -- cash_basis cash_basis_recognize transactions_draft` (38/38 pass).
- [ ] 3.5 `openspec archive a9-cash-basis-docs`.
