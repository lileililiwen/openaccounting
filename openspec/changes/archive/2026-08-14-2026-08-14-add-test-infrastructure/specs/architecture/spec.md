# architecture Specification (delta)

## ADDED Requirements

### Requirement: Library + Thin Binary Split

The application MUST be organised as a library crate
(`src/lib.rs`) plus a thin binary (`src/main.rs`). The library
exposes every application module publicly and provides a
`build_router(state, config) -> axum::Router` function that
constructs the full router (public + protected routes, auth
layer, session layer, trace layer) and a `run() -> anyhow::Result<()>`
function that performs the full process startup. The binary
contains only the call to `run()` and a pretty error handler.

This split exists so integration tests in `tests/` can drive the
real router through the public `build_router` entry point
without spawning a child process.

#### Scenario: Adding a new route from a test

- **WHEN** an integration test wants to call
  `GET /ledgers/{id}/reports/trial-balance` against the real
  application
- **THEN** it does so through `TestServer::new().await` and
  `client.get(format!("{base_url}/ledgers/{id}/reports/trial-balance"))`.
  The test is exercising the same `Router` instance that
  `cargo run` would serve, byte for byte.
