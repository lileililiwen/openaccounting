# quality Specification

## Purpose
TBD - created by archiving change fix-critical-bugs-and-quality. Update Purpose after archive.

## MODIFIED Requirements

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

#### Scenario: Build fails with unwrap

- **WHEN** a developer uses `.unwrap()` in a non-test source file
- **THEN** clippy flags it as a warning (with `-D warnings` it
  becomes an error).

### Requirement: No Unsafe Code

`unsafe` is forbidden in production crates via
`unsafe_code = "forbid"`. The crate root (`src/main.rs`) MUST
include `#![forbid(unsafe_code)]` to enforce this at compile time.
If a future change needs FFI, it MUST be in a new isolated crate
and MUST include a written justification in the change's `design.md`.

#### Scenario: PR adds `unsafe`

- **WHEN** a PR contains the `unsafe` keyword in `src/**`
- **THEN** `cargo build` fails with
  `error: usage of an unsafe attribute is forbidden`.

### Requirement: No `#[allow(dead_code)]`

`#[allow(dead_code)]` is **banned**. Dead code is real debt.
Restructure the code so every item is genuinely used, or delete the
dead path. The lint MUST NOT be silenced.

#### Scenario: Reviewer sees `#[allow(dead_code)]`

- **WHEN** a PR adds `#[allow(dead_code)]` to any item in `src/**`
- **THEN** the reviewer rejects the PR and asks the author to
  remove the dead code instead.

## ADDED Requirements

### Requirement: HTTP Header Sanitization

User-provided values MUST be sanitized before insertion into HTTP
headers. Sanitization MUST strip or escape characters that could
break the header format (double-quotes, control characters,
newlines).

#### Scenario: Filename with special characters

- **WHEN** a document has a filename containing `"`, `\n`, or
  other control characters
- **THEN** the Content-Disposition header uses a sanitized version
  of the filename that does not break the header format.
