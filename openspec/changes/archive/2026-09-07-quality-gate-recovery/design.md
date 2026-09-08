# Design

## Decisions

### Treat the current gate as a release blocker

The repository already declares `cargo clippy --all-targets -- -D warnings` and tests as mandatory. The implementation will fix warnings at their source rather than weakening the policy. This keeps the gate meaningful as the codebase grows.

### Separate product defects from environment failures

Pure unit tests must run without PostgreSQL. Database-backed tests may require PostgreSQL, but the test harness or wrapper must load `.env.test` deliberately and report the missing connection as setup guidance. A missing environment variable must not be mistaken for a domain failure.

### Preserve the import wizard's sentinel contract

`ColumnMap` uses `-1` for an unmapped field. `auto_detect` will map headers containing `amount`, `value`, or an equivalent supported amount label to `amount_in`, while debit/credit columns retain their existing precedence. Tests will cover both single-amount and debit/credit CSVs.

### Risks and mitigations

- Clippy cleanup may touch many modules; mitigate by grouping fixes by lint class and running the exact CI command after each group.
- Loading `.env.test` can surprise developers who intentionally set a different database; mitigate by giving an explicitly exported `DATABASE_URL` precedence.
- Amount-header heuristics can misclassify a debit column; mitigate with deterministic precedence and explicit mapping tests.

## Verification

Run `cargo fmt -- --check`, `cargo clippy --features test-support --all-targets -- -D warnings`, pure library tests, and the database-backed integration command with a fresh PostgreSQL database. The import-wizard regression test must pass before the broader suite is accepted.
