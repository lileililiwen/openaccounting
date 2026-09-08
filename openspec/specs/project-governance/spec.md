# project-governance Specification

## Purpose
TBD - created by archiving change spec-and-product-docs-alignment. Update Purpose after archive.
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

