# release-integrity Specification

## Purpose
States the reproducible-release contract: verification docs use the
canonical repository identity, published binaries carry tagged
provenance, quality gates pass before publication, and workflows pin
third-party actions. Used by release managers cutting a release and
by users verifying a downloaded artifact. Out of scope: deployment
topology and backup/restore procedures (see docs/production-deployment.md
and docs/backup-restore.md).

## ADDED Requirements

### Requirement: Durable release purpose

The Purpose of this spec SHALL state the reproducible-release
contract, its users, and its out-of-scope bounds instead of an
archiving placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the release contract, who uses it, and what remains out of scope.
