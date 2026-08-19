# reproducible-builds Specification (delta)

## ADDED Requirements

### Requirement: Bit-Identical Builds

MUST produce bit-identical binaries when built twice from the same source under the same pinned toolchain.

#### Scenario: Two builds

- **WHEN** the release workflow builds twice
- **THEN** the SHA256s match.

### Requirement: Signed Releases

MUST sign every release binary with cosign using a key whose public part is published in the repo.

#### Scenario: Verify

- **WHEN** the user runs `cosign verify-blob`
- **THEN** verification succeeds.

### Requirement: Pinned Toolchain

MUST use a pinned Rust toolchain (`rust-toolchain.toml` with a specific version) for release builds.

#### Scenario: Pinned

- **WHEN** the workflow checks out a specific rustc
- **THEN** the version is recorded in the build log.

### Requirement: Documentation

MUST publish a verification recipe in the README and a separate doc.

#### Scenario: Docs

- **WHEN** the README
- **THEN** the verification steps are present.
