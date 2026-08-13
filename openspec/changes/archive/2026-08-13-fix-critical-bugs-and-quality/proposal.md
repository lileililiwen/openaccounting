# Fix critical bugs and quality violations

## Why

An automated audit of the codebase against the OpenSpec capability
specs found 8 high-priority issues and 1 medium-priority issue:

- **H1 (CRITICAL):** Transaction form posts all fields to index 0,
  making multi-posting transactions impossible through the UI.
- **H2:** `ensure_owner` returns 403 instead of 404, leaking
  resource existence.
- **H3-H4:** Content-Disposition header injection in document
  download and CSV export.
- **H5:** `unwrap()` in production code (quality spec violation).
- **H6:** 4x `#[allow(dead_code)]` (quality spec bans this).
- **H7:** Missing `unsafe_code = "forbid"` (quality spec requires).
- **H8:** Missing `from > to` date validation in reports.
- **M3:** General ledger running balance SQL has a no-op
  `CASE WHEN ... THEN 0 ELSE 0`.

All of these are bugs or spec violations that need to be fixed
before adding new features.

## What Changes

- Fix transaction form template: use `loop.index0` for posting
  field indices, update JS clone handler.
- Change `ensure_owner` to return `AppError::NotFound`.
- Sanitize filenames in Content-Disposition headers.
- Remove `unwrap()` in dashboard redirect.
- Remove all `#[allow(dead_code)]` items.
- Add `#![forbid(unsafe_code)]` to main.rs.
- Add `from > to` validation in report handlers.
- Fix general ledger SQL no-op.

## Capabilities

### Modified Capabilities

- `bookkeeping` — fix transaction form posting indices.
- `reports` — add date range validation, fix general ledger SQL.
- `quality` — remove dead code, unwrap, add unsafe_code forbid.

## Impact

- **Modified files:** `templates/transactions/new.html`,
  `src/handlers/ledgers.rs`, `src/handlers/documents.rs`,
  `src/handlers/reports.rs`, `src/handlers/dashboard.rs`,
  `src/handlers/account.rs`, `src/reports/general_ledger.rs`,
  `src/main.rs`, `static/app.js`.
- **Database:** none.
- **Backwards compatible:** all changes are bug fixes, no API
  changes.

## Non-Goals

- CSRF protection (separate change).
- Pagination (separate change).
- Test suite expansion (separate change).
- Config hardcoding (separate change).
