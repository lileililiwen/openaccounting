# Design

## Decisions

### Use the real router and fresh databases

The smoke suite will use `TestServer` and `TestDb`, exercising the same Axum router as production. Each test gets an isolated database and temporary document storage so order and prior state cannot affect results.

### Keep smoke coverage narrow and high-value

The golden path will cover registration, login, ledger creation, balanced posting, rejection of an unbalanced posting, document upload/download, representative reports, export, and logout. Detailed feature behavior remains in focused integration files.

### Use property tests for invariants, not endpoints

The accounting balance rule, account sign mapping, and filename sanitization are pure invariants. Property tests will generate boundary and randomized values, with at least 100 cases per invariant where the current project policy requires it.

### Risks and mitigations

- PostgreSQL availability can block local execution; mitigate with a documented CI service and a clear setup check.
- A single smoke path can become brittle; mitigate by asserting stable status codes and semantic markers rather than full HTML snapshots.
- Property generators can miss decimal edge cases; mitigate with explicit zero, large, negative, and multi-leg examples alongside generated cases.

## Verification

Run the smoke test twice against fresh databases, run focused property tests, run the full integration suite, and inspect the coverage artifact. The test suite must be order-independent.
