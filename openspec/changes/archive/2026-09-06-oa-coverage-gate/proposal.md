# Proposal: Add a coverage gate to CI
## Why
ci.yml runs fmt, clippy -D warnings, Postgres-backed tests, and a reversible-migrations job, but has no coverage gate - clippy passing does not guarantee test coverage.
## What Changes
- Add a coverage job (cargo-llvm-cov or tarpaulin) with a published artifact and a floor threshold.

## Capabilities
### New Capabilities
- `oa-coverage-gate`: minimum test coverage is enforced in CI.

### Modified Capabilities
None.

## Impact
Affects: .github/workflows/ci.yml.
