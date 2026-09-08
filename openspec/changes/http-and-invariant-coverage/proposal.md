# HTTP and invariant coverage

## Why

The repository contains many integration tests, but its testing specification requires dedicated HTTP smoke coverage, property tests for accounting invariants, and repeatable execution. The expected `tests/http/` and `tests/smoke.rs` surfaces are absent, so important claims are not represented by a stable golden-path gate.

## What changes

- Add capability `test-coverage`.
- Add a repeatable HTTP smoke path against the real router and fresh PostgreSQL database.
- Add property tests for posting balance, account normal-direction mapping, and filename sanitization.
- Add explicit coverage for authorization boundaries, uploads, exports, reports, and authentication transitions.
- Keep the existing coverage floor, but make the most important business invariants independently visible.

## Non-goals

- No browser automation framework unless the existing HTTP fixture cannot exercise a requirement.
- No arbitrary line-coverage target increase in this change.
- No replacement of the existing integration suite.
