# test-coverage Specification

## Purpose
States the test-suite contract: a golden-path HTTP smoke test,
double-entry property tests, security and data-boundary HTTP
coverage, and order-independent repeatability. Used by contributors
adding suites and by CI enforcing them. Out of scope: the numeric
coverage floor, which belongs to oa-coverage-gate.
## Requirements
### Requirement: Golden-path HTTP smoke test

The project SHALL provide a smoke test against the real Axum router and fresh PostgreSQL database covering the minimum user journey from registration through logout.

#### Scenario: Repeatable bookkeeping journey

- **WHEN** the smoke test runs twice against fresh databases
- **THEN** both runs assert successful registration, login, ledger creation, balanced transaction creation, document upload/download, representative report responses, export, and logout with no manual cleanup.

### Requirement: Double-entry property coverage

The project SHALL property-test that balanced multi-leg postings are accepted and unbalanced postings are rejected.

#### Scenario: Random balanced postings

- **WHEN** generated debit and credit partitions have equal sums
- **THEN** the domain validator accepts them across at least 100 generated cases.

#### Scenario: Random unbalanced postings

- **WHEN** generated debit and credit partitions have different sums
- **THEN** the domain validator rejects them with the expected domain error across at least 100 generated cases.

### Requirement: Security and data-boundary HTTP coverage

The HTTP suite SHALL cover cross-user access rejection for ledgers/documents, CSRF rejection, upload validation, API authentication, and export authorization.

#### Scenario: Cross-user document access

- **WHEN** an authenticated user requests a document owned by another user
- **THEN** the response does not disclose the document and uses the documented authorization status.

### Requirement: Test repeatability

The test commands SHALL be order-independent and SHALL not rely on pre-existing database rows or document files.

#### Scenario: Isolated test execution

- **WHEN** a focused test is run alone and then as part of the full suite
- **THEN** it produces the same assertions and leaves no required persistent state.

### Requirement: Durable coverage purpose

The Purpose of this spec SHALL state the test-suite contract, its
users, and its out-of-scope bounds instead of an archiving
placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the test contract, who uses it, and what remains out of scope.

