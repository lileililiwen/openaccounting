# project-governance Specification

## Purpose
States the contributor-facing documentation contract: README feature
status stays unambiguous, architecture docs match the source tree,
accepted specs keep durable purposes, and doc links resolve. Used by
contributors and agents writing or reviewing changes. Out of scope:
choosing an open-source foundation or CLA (see GOVERNANCE.md, which
covers process only).

## ADDED Requirements

### Requirement: Durable contributor purpose

The Purpose of this spec SHALL state the contributor documentation
contract, its users, and its out-of-scope bounds instead of an
archiving placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the documentation contract, who uses it, and what remains out of scope.
