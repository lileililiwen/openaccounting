# Cash-Basis Toggle — Tasks

## 1. Testing

- [x] 1.1 Unit: `ReportBasis::parse` accepts valid values and
      rejects invalid ones (covered by the `basis=foo → 400`
      integration test; the parse fn itself is exercised
      end-to-end).
- [x] 1.2 Integration: `http_income_statement_accrual_includes_ar_revenue`.
- [x] 1.3 Integration: `http_income_statement_cash_excludes_ar_revenue`
      — assert totals and the excluded footer.
- [x] 1.4 Integration: `http_income_statement_basis_invalid_returns_400`.
- [x] 1.5 Integration: `http_ledger_create_with_cash_basis_persists`.
- [x] 1.6 Integration: `http_cash_flow_footer_shows_basis`.

## 2. Implementation

- [x] 2.1 Migration `0020_add_ledger_basis.sql` (single
      `ALTER TABLE ledgers ADD COLUMN basis …`).
- [x] 2.2 `src/reports/mod.rs` — `ReportBasis` enum + `parse`.
- [x] 2.3 `src/domain/ledger.rs` — add `basis` field to `Ledger`.
- [x] 2.4 `src/reports/income_statement.rs` — `run(..., basis)`
      and an `ExcludedTotals` footer; one round-trip computes
      both totals.
- [x] 2.5 `src/reports/cash_flow.rs` — accept `basis` parameter
      (semantics unchanged, recorded in footer).
- [x] 2.6 `src/handlers/reports.rs` — parse `basis` query
      param; pass through; render footer text.
- [x] 2.7 `src/handlers/ledgers.rs` — accept `basis` on
      `create`.
- [x] 2.8 `templates/reports/income_statement.html` — basis
      toggle.
- [x] 2.9 `templates/reports/cash_flow.html` — basis toggle.
- [x] 2.10 `templates/ledgers/new.html` — basis selector on
      the create form.
- [x] 2.11 Update `openspec/specs/reports/spec.md` archive with
      the new requirements (after archive step).
- [x] 2.12 Add a "Cash-basis toggle" bullet to the README so
      the feature is documented.

## 3. Validation

- [x] 3.1 `openspec validate cash-basis-toggle` passes.
- [x] 3.2 `cargo fmt --check` clean (on changed files).
- [x] 3.3 `cargo clippy --all-targets --features test-support`
      introduces no new warnings in the files this change
      touches.
- [x] 3.4 `cargo test --features test-support` green
      (11 lib + 3 smoke + 5 cash-basis = 19 tests).
- [x] 3.5 Manual smoke: create ledger with `basis=cash`, post an
      AR sale, run income statement — assert zero revenue
      (covered by `http_income_statement_cash_excludes_ar_revenue`).
- [x] 3.6 README: added the "Cash-basis toggle" bullet so the
      feature is documented.
- [x] 3.7 `openspec archive cash-basis-toggle`.
