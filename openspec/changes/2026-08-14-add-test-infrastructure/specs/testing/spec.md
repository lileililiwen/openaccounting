# testing Specification (delta)

## ADDED Requirements

### Requirement: Per-Test Database and HTTP Server Fixture

The `tests/` directory MUST contain a per-test fixture that gives
every integration test a fresh PostgreSQL database and a real
axum server on a random local port. The fixture lives behind the
`test-support` Cargo feature and is re-exported from
`openaccounting::test_support` and from `tests/common/mod.rs`.

#### Scenario: Test gets a fresh database

- **WHEN** an integration test calls `TestDb::new().await`
- **THEN** a brand-new PostgreSQL database with a UUID-suffixed
  name is created, every migration in `migrations/` is applied,
  and the pool is returned. The database is dropped when the
  `TestDb` value is dropped.

#### Scenario: Test gets a real axum server

- **WHEN** an integration test calls `TestServer::new().await`
- **THEN** the real application router from
  `openaccounting::build_router` is bound to `127.0.0.1:0`, the
  bound port is captured, and a `reqwest::Client` with cookie
  support is returned. The server is aborted on drop.

#### Scenario: Test can register and log in

- **WHEN** a test calls
  `TestServer::bootstrap_user(username, email, password).await`
- **THEN** the user is registered, the session cookie is captured
  from the `Set-Cookie` header of the login response, and the
  cookie value is returned so the test can drive subsequent
  authenticated requests with `Cookie: oa_session=<value>`.

#### Scenario: TestDb and TestServer are dev-only

- **WHEN** the project is built with `cargo build --release`
  (no `test-support` feature)
- **THEN** `openaccounting::test_support` is not present in the
  compiled binary; it adds no symbols, no dependencies, and no
  startup cost.
