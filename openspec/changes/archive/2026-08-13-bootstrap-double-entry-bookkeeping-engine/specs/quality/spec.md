# quality Specification (delta)

## ADDED Requirements

### Requirement: No Panics in Production Code

`unwrap`, `expect`, `panic!`, `todo!`, and `unimplemented!` are
**forbidden in production code** (anything not under `#[cfg(test)]`).
Use one of:

- `?` to propagate the error.
- `match` to handle expected cases.
- `.expect("invariant: <description>")` where the invariant is
  provably maintained at the call site; the message MUST describe
  the invariant, not the panic.

This rule is enforced by `cargo clippy -- -D warnings` with the
`clippy.toml` policy described in `AGENTS.md §5`.

#### Scenario: A reviewer sees `.unwrap()`

- **WHEN** a PR contains `.unwrap()` in `src/handlers/` or
  `src/main.rs` outside `#[cfg(test)]`
- **THEN** the reviewer MUST block the PR until the call is
  converted to `?`, `match`, or `.expect("invariant: …")`.

### Requirement: No Unsafe Code

`unsafe` is forbidden in production crates via
`unsafe_code = "forbid"`. If a future change needs FFI, it MUST be
in a new isolated crate and MUST include a written justification in
the change's `design.md`.

#### Scenario: PR adds `unsafe`

- **WHEN** a PR contains the `unsafe` keyword in `src/**`
- **THEN** `cargo build` fails with
  `error: usage of an `unsafe` attribute is forbidden`.

### Requirement: No `#[allow(dead_code)]`

`#[allow(dead_code)]` is **banned**. Dead code is real debt.
Restructure the code so every item is genuinely used, or delete the
dead path. The lint MUST NOT be silenced.

#### Scenario: Reviewer sees `#[allow(dead_code)]`

- **WHEN** a PR adds `#[allow(dead_code)]` to any item
- **THEN** the reviewer MUST request the dead code be removed or
  used. If it is genuinely useful, the reviewer can suggest moving
  it to a single shared test module that compiles once per test
  binary, but the `allow` itself is still rejected.

### Requirement: `cargo fmt --check` Clean

Every commit MUST leave the workspace in a state where
`cargo fmt --all -- --check` exits 0.

#### Scenario: CI runs `cargo fmt --check`

- **WHEN** CI runs `cargo fmt --check`
- **THEN** the build fails if any file is not formatted. The
  developer runs `cargo fmt` and re-pushes.

### Requirement: `cargo clippy -D warnings` Clean

Every commit MUST leave the workspace in a state where
`cargo clippy --workspace --all-targets -- -D warnings` exits 0.

#### Scenario: Reviewer runs clippy

- **WHEN** `make check` (which runs clippy) is invoked
- **THEN** it MUST exit 0 before a PR can merge.
