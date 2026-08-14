# Cash-Basis Toggle — Tasks

## 1. Testing

- [ ] 1.1 Unit: `ReportBasis::parse` accepts valid values and
      rejects invalid ones.
- [ ] 1.2 Integration: `http_income_statement_accrual_includes_ar_revenue`.
- [ ] 1.3 Integration: `http_income_statement_cash_excludes_ar_revenue`
      — assert totals and the excluded footer.
- [ ] 1.4 Integration: `http_income_statement_basis_invalid_returns_400`.
- [ ] 1.5 Integration: `http_ledger_create_with_cash_basis_persists`.
- [ ] 1.6 Integration: `http_cash_flow_footer_shows_basis`.

## 2. Implementation

- [ ] 2.1 Migration `0020_add_ledger_basis.sql` (single
      `ALTER TABLE ledgers ADD COLUMN basis …`).
- [ ] 2.2 `src/reports/mod.rs` — `ReportBasis` enum + `parse`.
- [ ] 2.3 `src/domain/ledger.rs` — add `basis` field to `Ledger`.
- [ ] 2.4 `src/reports/income_statement.rs` — split into
      `run_accrual()` and `run_cash()`, both `pub`.
- [ ] 2.5 `src/reports/cash_flow.rs` — accept `basis` parameter
      (semantics unchanged, recorded in footer).
- [ ] 2.6 `src/handlers/reports.rs` — parse `basis` query
      param; pass through; render footer text.
- [ ] 2.7 `src/handlers/ledgers.rs` — accept `basis` on
      `create`.
- [ ] 2.8 `templates/reports/income_statement.html` — basis
      toggle.
- [ ] 2.9 `templates/reports/cash_flow.html` — basis toggle.
- [ ] 2.10 `templates/ledgers/new.html` — basis selector on
      the create form.
- [ ] 2.11 Update `openspec/specs/reports/spec.md` archive with
      the new requirements (after archive step).
- [ ] 2.12 Rewrite the cash-basis paragraph in `README.md` so
      it describes the toggle introduced by this change rather
      than a feature that already exists.

## 3. Validation

- [ ] 3.1 `openspec validate cash-basis-toggle` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: create ledger with `basis=cash`, post an
      AR sale, run income statement — assert zero revenue.
- [ ] 3.6 README smoke: read the cash-basis paragraph in
      `README.md` and confirm it now reads as a description of
      the toggle being introduced by this change, not a claim
      that the feature already exists.
- [ ] 3.7 `openspec archive cash-basis-toggle`.
