# project-governance Specification

## Purpose
States the contributor-facing documentation contract: README feature
status stays unambiguous, architecture docs match the source tree,
accepted specs keep durable purposes, and doc links resolve. Used by
contributors and agents writing or reviewing changes. Out of scope:
choosing an open-source foundation or CLA (see GOVERNANCE.md, which
covers process only).
## Requirements
### Requirement: Feature status is unambiguous

The README SHALL classify each advertised capability as shipped, optional, experimental, provider-dependent, or deferred, and SHALL NOT list a shipped capability as a v1 non-goal.

#### Scenario: Feature and non-goal review

- **WHEN** a contributor compares the Features and Non-Goals sections
- **THEN** no capability appears in both sections without an explicit status explanation.

### Requirement: Architecture documentation matches source

The architecture specification SHALL describe the actual library/binary split, composition-root responsibilities, and dependency versions used by `Cargo.toml`.

#### Scenario: Composition root check

- **WHEN** a contributor follows the architecture specification to locate router construction
- **THEN** it identifies `src/lib.rs` for router composition and `src/main.rs` for process startup, consistently with the source.

### Requirement: Capability specifications have durable purposes

Every accepted capability spec SHALL have a meaningful Purpose section that describes the capability's current contract and scope.

#### Scenario: Archived capability is discoverable

- **WHEN** a future agent opens a capability spec without its archived change
- **THEN** the Purpose explains what the capability does, who uses it, and what remains out of scope.

### Requirement: Documentation links are valid

Current README and release documentation SHALL reference the actual repository, existing paths, supported commands, and current configuration names.

#### Scenario: Fresh checkout quick start

- **WHEN** a contributor follows the README from a fresh checkout
- **THEN** every referenced file and command exists or is explicitly marked optional.

### Requirement: Durable contributor purpose

The Purpose of this spec SHALL state the contributor documentation
contract, its users, and its out-of-scope bounds instead of an
archiving placeholder.

#### Scenario: Purpose survives its change

- **WHEN** a future agent opens this spec without its archived change
- **THEN** the Purpose explains the documentation contract, who uses it, and what remains out of scope.

