# testing Specification (delta)

## ADDED Requirements

### Requirement: Test Categories and Locations

Tests MUST be organized into the following categories, each with a
fixed location and naming convention:

| Category   | Location                                         | Naming                          |
|------------|--------------------------------------------------|---------------------------------|
| Unit       | `src/<area>.rs` inside `#[cfg(test)] mod tests`  | `tests::test_<unit>`            |
| Property   | `src/<area>.rs` inside `mod prop`                | `prop_<invariant>`              |
| Integration| `tests/integration/<area>.rs`                    | `<area>_<behavior>`             |
| HTTP       | `tests/http/<flow>.rs`                           | `http_<flow>_<scenario>`        |

A single behaviour MUST have at most one test in each category that
covers it, so the question "does this work?" can be answered at
three scopes without duplication.

#### Scenario: Adding a new function `build_trial_balance`

- **WHEN** the developer adds `build_trial_balance` in
  `src/reports/trial_balance.rs`
- **THEN** they MUST add at least one unit test in
  `#[cfg(test)] mod tests` covering the in-memory aggregation logic.

### Requirement: Repeatability

Tests MUST NOT depend on:

- A running PostgreSQL daemon (use the `TestDb` helper, which spins
  up a unique database per test in CI; in dev it requires a running
  Postgres).
- The current contents of any DB (each test gets a fresh DB).
- Filesystem state from a previous test (`TempDir` per test).
- Environment variables set outside the test.

`cargo test --workspace` MUST be order-independent and run twice in
a row with identical results.

#### Scenario: Test runs in isolation

- **WHEN** a single test is run via `cargo test --test foo test_x`
- **THEN** it passes without any prior setup, no other tests
  running, and no environment variables beyond `DATABASE_URL`.

### Requirement: Property-Based Tests for Invariants

The double-entry invariant MUST have a property test:

- For any random partition of a non-negative amount into `N ≥ 2`
  parts, half of which are debits and half credits, the validator
  accepts iff the sum of debits equals the sum of credits.
- For any random pair `(debits, credits)` where they are NOT equal,
  the validator rejects with the expected error.

The chart of accounts `AccountType::normal_direction` mapping MUST
have a property test: for each of the 5 variants, applying the
mapping and the report code produces the expected sign.

The `sanitize-filename` step MUST have a property test: for any
random input string, the result contains no `..`, no `/`, no `\`,
no leading `.`, and is non-empty.

#### Scenario: Random unbalanced partition is rejected

- **WHEN** `prop_posting_balance` is run with 100 random cases
- **THEN** every case where `Σ debits ≠ Σ credits` is rejected and
  every case where they are equal is accepted.

### Requirement: HTTP Smoke Tests

The HTTP layer MUST have a smoke test that exercises the full
golden path. The test SHALL run against a real Postgres and the
binary listening on the port specified by `OA_TEST_PORT` (default
`3001`).

#### Scenario: Golden path is repeatable

- **WHEN** the smoke test in `tests/smoke.rs` is run twice in a row
  against a fresh test database
- **THEN** both runs pass identically, no manual cleanup is needed
  between runs, and the same assertions hold.

#### Scenario: Each step asserts something concrete

- **WHEN** the smoke test runs
- **THEN** every step in the path (register, login, ledger, balanced
  transaction, unbalanced rejection, upload, download, five
  reports, logout) has at least one explicit assertion on the
  response status and (where relevant) the response body.
