# release-integrity Specification

## Purpose
States the reproducible-release contract: verification docs use the
canonical repository identity, published binaries carry tagged
provenance, quality gates pass before publication, and workflows pin
third-party actions. Used by release managers cutting a release and
by users verifying a downloaded artifact. Out of scope: deployment
topology and backup/restore procedures (see docs/production-deployment.md
and docs/backup-restore.md).
## Requirements
### Requirement: Repository-correct verification

All release verification examples and generated release instructions SHALL use the canonical repository identity and SHALL not contain a stale repository owner or name.

#### Scenario: User verifies a release

- **WHEN** a user copies the verification command from the README or release notes
- **THEN** the Cosign certificate identity matches the workflow that produced the downloaded artifact.

### Requirement: Tagged release provenance

Every published binary SHALL identify the source commit, version tag, Rust toolchain, SHA256 checksum, and signature bundle.

#### Scenario: Release tag is moved or mismatched

- **WHEN** the release workflow is requested for a tag that does not resolve to the checked-out commit
- **THEN** the workflow fails before publishing artifacts.

### Requirement: Release quality gates

The release workflow SHALL require formatting, lint, unit/integration tests, migration verification, reproducibility verification, and dependency-audit status before publication.

#### Scenario: Quality gate failure

- **WHEN** any required gate fails
- **THEN** no GitHub Release or signed binary is published by that workflow run.

### Requirement: Action supply-chain pinning

Release and CI workflows SHALL pin third-party actions to immutable reviewed references or enforce an equivalent repository policy.

#### Scenario: Action reference review

- **WHEN** a workflow is changed
- **THEN** every external action reference is either commit-pinned or covered by the documented exception policy.

### Requirement: Durable release purpose

The Purpose of this spec SHALL state the reproducible-release
contract, its users, and its out-of-scope bounds instead of an
archiving placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the release contract, who uses it, and what remains out of scope.

