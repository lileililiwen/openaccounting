# Quality-gate recovery

## Why

The repository's required quality gate is currently red. Clippy reports a large set of warnings promoted to errors, and the library suite contains a deterministic CSV auto-detection failure. Database-backed tests also fail unclearly when the developer has not exported `DATABASE_URL`, while the repository documents `.env.test` as the source of that configuration.

## What changes

- Add capability `quality-gate-recovery`.
- Restore a clean `cargo fmt`, Clippy, unit-test, and test-support baseline.
- Make the CSV wizard detect a supported single amount column.
- Make local and CI test configuration explicit and fail with actionable setup guidance.
- Ensure CI stops at the first failed gate and exposes the same commands contributors use locally.

## Non-goals

- No new product feature beyond correcting the existing import-wizard behavior.
- No broad refactor of all high-complexity functions.
- No lowering of Clippy policy or removal of existing tests to make the gate pass.
