# quality-gates Specification

## Purpose

Ensures the project maintains a clean formatting, Clippy, unit, and integration test baseline. Includes CSV import auto-detection for single-amount columns, production credential validation, and documented verification commands.
## Requirements
### Requirement: Required quality commands pass

The project SHALL keep formatting, Clippy, unit, and integration commands green under the same toolchain and feature combinations used by CI.

#### Scenario: Clean lint gate

- **WHEN** a contributor runs `cargo fmt -- --check` and `cargo clippy --features test-support --all-targets -- -D warnings`
- **THEN** both commands exit successfully without changing files or suppressing existing lint classes.

### Requirement: CSV amount auto-detection

The CSV wizard SHALL map a supported single amount header to `ColumnMap.amount_in` and SHALL leave unrelated optional fields at the `-1` sentinel.

#### Scenario: Single amount column

- **WHEN** headers are `Txn Date`, `Description`, and `Amount`
- **THEN** the date index is `0`, description index is `1`, amount index is `2`, and debit/credit indexes remain unmapped.

#### Scenario: Debit and credit columns

- **WHEN** headers contain separate debit and credit columns
- **THEN** debit and credit mapping remains correct and is not replaced by a synthetic amount mapping.

### Requirement: Test setup is actionable

The database-backed test entry point SHALL load `.env.test` when no `DATABASE_URL` is already exported and SHALL report the required PostgreSQL privilege and connection requirements when setup fails.

#### Scenario: Local test without exported URL

- **WHEN** a contributor runs the documented database-backed test command from the repository root without an exported `DATABASE_URL`
- **THEN** the harness uses `.env.test` if present, or returns an actionable error naming the missing variable and required database capability.

### Requirement: CI does not hide failed gates

The CI workflow SHALL execute quality gates as independent fail-fast steps and SHALL use the same feature flags and test commands documented for local verification.

#### Scenario: Failing lint blocks tests from being reported as green

- **WHEN** Clippy fails
- **THEN** the workflow is unsuccessful and cannot publish a successful quality result.

