# Fix critical bugs and quality — Tasks

> **Spec-first rule (from `Agents.md §2.4`):** `## 1. Testing` comes
> first. Tests are written and red **before** any `## 2.
> Implementation` task is marked complete.

## 1. Testing

- [ ] 1.1 Unit: `sanitize_header_value` strips control chars and
      double-quotes from filenames.
- [ ] 1.2 Unit: report handlers return `400` when `from > to`.
- [ ] 1.3 Unit: `ensure_owner` returns `NotFound` (not Forbidden)
      for non-owned ledgers.

## 2. Implementation

- [ ] 2.1 **H1:** Fix `templates/transactions/new.html` — use
      `{% set idx = loop.index0 %}` and `lines[{{ idx }}][field]`
      for all posting inputs. Update `static/app.js` clone handler
      to rewrite indices.
- [ ] 2.2 **H2:** Change `ensure_owner` in
      `src/handlers/ledgers.rs` from `AppError::Forbidden` to
      `AppError::NotFound`.
- [ ] 2.3 **H3-H4:** Add `sanitize_header_value(s: &str) -> String`
      helper that strips control chars and `"`. Apply in
      `src/handlers/documents.rs` (download) and
      `src/handlers/reports.rs` (CSV export).
- [ ] 2.4 **H5:** Replace `Body::empty().unwrap()` in
      `src/handlers/dashboard.rs` with proper error handling.
- [ ] 2.5 **H6:** Delete dead code:
      - `src/handlers/ledgers.rs:183` `now()` function
      - `src/handlers/account.rs:83` `_user_marker()` function
      - `src/reports/general_ledger.rs:129` `type_for_total()`
        function (check if actually dead first)
      - `src/templates/account.rs:96` `_UUID_MARKER` constant
- [ ] 2.6 **H7:** Add `#![forbid(unsafe_code)]` to `src/main.rs`.
- [ ] 2.7 **H8:** Add date range validation in report handlers:
      if `from > to`, return `AppError::Validation("from must be <= to")`.
- [ ] 2.8 **M3:** Remove the no-op
      `CASE WHEN a.type IN ('ASSET','EXPENSE') THEN 0 ELSE 0 END`
      from general ledger SQL.

## 3. Validation

- [ ] 3.1 `openspec validate fix-critical-bugs-and-quality` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets` — no new warnings in files
      touched by this change.
- [ ] 3.4 `cargo build` succeeds.
- [ ] 3.5 All tasks under `## 1. Testing` are green.
- [ ] 3.6 Manual smoke: create a 2-posting transaction via the UI
      → verify both postings are saved.
- [ ] 3.7 `openspec archive fix-critical-bugs-and-quality`.
