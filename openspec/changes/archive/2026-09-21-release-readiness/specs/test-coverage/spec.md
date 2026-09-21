# test-coverage Specification

## Purpose
States the test-suite contract: a golden-path HTTP smoke test,
double-entry property tests, security and data-boundary HTTP
coverage, and order-independent repeatability. Used by contributors
adding suites and by CI enforcing them. Out of scope: the numeric
coverage floor, which belongs to oa-coverage-gate.

## ADDED Requirements

### Requirement: Durable coverage purpose

The Purpose of this spec SHALL state the test-suite contract, its
users, and its out-of-scope bounds instead of an archiving
placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the test contract, who uses it, and what remains out of scope.
