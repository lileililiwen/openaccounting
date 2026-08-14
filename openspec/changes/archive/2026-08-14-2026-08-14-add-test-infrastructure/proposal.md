# Add Test Infrastructure (Per-Test DB, HTTP Test Server)

## Why

There is no `tests/` directory in the repository. All existing tests
live in `#[cfg(test)] mod tests` blocks inside source files (see
`src/auth/mod.rs`). They require a developer to point `DATABASE_URL`
at a live Postgres, share that database across every test in the
process, and run a single test by hand.

This blocks the implementation of every other OpenSpec change
currently parked in `openspec/changes/`. The 13 in-flight changes
all assume the existence of a `TestServer` / `TestDb` fixture that
the test can spin up in isolation, drive with `reqwest`, and tear
down deterministically. Without it, the spec tasks for any change
that has an HTTP integration test cannot be marked done.

The pattern in `/home/paul/code/openpanel` (specifically
`crates/openpanel-test-support/`) is the reference: per-test DB,
real axum router on a random port, `reqwest` client, RAII cleanup,
single central entry point (`tests/integration/main.rs`). We mirror
that pattern, adapted to a Postgres-backed single binary (instead
of a SQLite-backed workspace).

This is a foundation change. It does not deliver any user-visible
feature; it delivers the ability to verify the other changes.

## What Changes

- Refactor `src/main.rs` into a thin `[[bin]]` that calls into a
  new `src/lib.rs` exposing the same modules. This lets
  integration tests in `tests/` `use openaccounting::…`.
- Add `src/test_support.rs` (gated behind a `test-support`
  Cargo feature so it is invisible to production builds) with:
  - `TestDb` — RAII wrapper around a per-test PostgreSQL database.
    `new()` parses the admin URL from `DATABASE_URL`, generates a
    unique `oa_test_<uuid>` name, runs `CREATE DATABASE`, connects,
    runs all migrations, returns the pool. `Drop` runs
    `DROP DATABASE` on a fresh admin connection.
  - `TestServer` — boots the real axum router (via the new
    `build_router(state, session_secret)` entry point in the
    library) on `127.0.0.1:0`, returns base URL, `reqwest` client
    (with cookie store), and a `bootstrap_user(username, password)`
    helper that registers, logs in, and returns the session cookie
    value.
  - `cfg(test)` or `feature = "test-support"` only.
- Add the `test-support` feature to `Cargo.toml` and declare
  `reqwest` and `tempfile` as `[dev-dependencies]`.
- Add `tests/common/mod.rs` that re-exports
  `openaccounting::test_support::*`.
- Add `tests/integration/main.rs` as the single binary entry point
  for the `integration` test, with `mod smoke;` and any future
  area modules.
- Add `tests/integration/smoke.rs` with two tests: server boots +
  the public routes (`/login`, `/register`) return 200, and
  `/static/htmx.min.js` returns 200.
- Add `openspec/specs/testing/spec.md` requirement for the
  `TestDb` / `TestServer` contract.

## Capabilities

### New Capabilities

- `testing` — add a requirement to `openspec/specs/testing/spec.md`
  describing the `TestDb` / `TestServer` contract.

### Modified Capabilities

- `architecture` — replace the "composition root is `main.rs`"
  requirement with one that distinguishes composition
  (`build_router`) from process startup (`main.rs`), and notes that
  `lib.rs` exposes modules for both the binary and the integration
  tests.

## Impact

- **New files:**
  - `src/lib.rs`
  - `src/test_support.rs`
  - `tests/common/mod.rs`
  - `tests/integration/main.rs`
  - `tests/integration/smoke.rs`
- **Modified files:**
  - `Cargo.toml` (add `test-support` feature, dev-dependencies).
  - `src/main.rs` (thin shell, calls into `lib.rs`).
  - `openspec/specs/testing/spec.md` (new requirement).
  - `openspec/specs/architecture/spec.md` (composition-root
    update).
  - `Agents.md` (note: integration tests now live under `tests/`
    with a `TestServer` fixture; the `#[cfg(test)]` pattern
    described in §4 is unchanged for unit tests).

## Non-Goals

- Property-based test infrastructure (`proptest` is not added in
  this change; the `testing` spec mentions it but a follow-up
  change can add the `proptest` crate when there is a domain
  invariant that needs it).
- GitHub Actions / CI config (out of scope; this change only adds
  the local infra).
- Performance / parallel-test optimisation. Tests are designed to
  run with `--test-threads=1` to avoid Postgres lock contention.
  A follow-up can introduce per-test schema-per-pool if needed.
- Mocks for the Postgres connection. The fixture uses a real
  Postgres; that is the project rule (per
  `openspec/specs/testing/spec.md`).

## Dependencies & Pre-conditions

- A running PostgreSQL reachable via `DATABASE_URL` with a user
  that has `CREATEDB` privilege.
- No change to the application schema or behaviour.
