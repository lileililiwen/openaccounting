# architecture Specification (delta)

## ADDED Requirements

### Requirement: Layered Architecture

The system SHALL be organized into the following layers with strict
dependency direction. Inner layers MUST NOT depend on outer layers.

1. **Domain** (`src/domain/`) — entities, value objects, invariants,
   repository queries (sqlx as a type provider only). No `axum`, no
   `tokio`, no `askama`. No I/O.
2. **Reports** (`src/reports/`) and **Charts** (`src/charts/`) —
   pure computation; may use `sqlx` to query. No `axum`, no
   `askama`.
3. **Storage** (`src/storage/`) — document I/O. May use `tokio::fs`.
   No `axum`.
4. **Auth** (`src/auth/`) — password hashing, session store. May use
   `axum-login` and `sqlx`. No business logic.
5. **Handlers** (`src/handlers/`) — HTTP transport. May use any layer
   below. Returns `Result<impl IntoResponse, AppError>`.
6. **Templates** (`src/templates/`) — Askama structs that handlers
   render. May import from any layer.
7. **Composition root** (`src/main.rs`) — the only file that
   constructs `PgPool`, `FilesystemStore`, `AppState`, and wires the
   router.

#### Scenario: Adding a new handler

- **WHEN** a developer adds a new resource (e.g. `budgets`)
- **THEN** they create `src/handlers/budgets.rs`, optionally a new
  module under `src/reports/`, and register the route in `main.rs`.
  They MUST NOT touch `src/domain/` unless adding a new entity.

#### Scenario: Domain code stays I/O-free

- **WHEN** `grep -RE 'use (axum|tokio|askuma)' src/domain/` is run
- **THEN** the output is empty (the only allowed external imports are
  `sqlx::FromRow`, `serde`, `uuid`, `chrono`, `rust_decimal`).

### Requirement: Tech Stack Pinning

The system SHALL use the following pinned dependencies in `Cargo.toml`:

- `axum = "0.8.1"` with `features = ["multipart"]`
- `sqlx = "0.8.3"` with `features = ["postgres", "runtime-tokio",
  "tls-rustls", "macros", "uuid", "chrono", "migrate", "rust_decimal"]`
- `argon2 = "0.5.3"` for password hashing
- `axum-login = "0.17.0"` for auth manager
- `tower-sessions = "0.14.0"` and
  `tower-sessions-sqlx-store = { version = "0.16.0", features = ["postgres"] }`
  for session storage
- `askama = "0.13.0"` and `askama_axum = "0.14.0"` for templates
- `rust_decimal = "1.36.0"` with `features = ["serde-with-str"]` for
  money

Any new dependency MUST be added with a justified entry in the
`design.md` Decisions section of its change.

#### Scenario: Reviewer spots a new dependency

- **WHEN** a PR introduces a new top-level dependency
- **THEN** the PR includes a `design.md` update explaining the choice
  and at least one alternative considered.

### Requirement: Single Binary Output

The system SHALL compile to a single static binary
(`target/release/openaccounting`) that contains the entire web
application. There is no separate API server, worker, or CLI. The
binary reads configuration from environment variables (see
`AGENTS.md §6`) and connects to a single PostgreSQL database.

#### Scenario: Operator deploys

- **WHEN** an operator runs `docker compose up -d` (or equivalent)
- **THEN** exactly one application container and one postgres
  container are running. No additional services are required.

### Requirement: Configuration Validation

On startup, the binary MUST validate that:

- `DATABASE_URL` is parseable as a Postgres URL.
- `APP_SECRET` is at least 32 characters.
- `DOCUMENTS_DIR` either exists or can be created.

If any check fails, the process exits with a clear error message and
a non-zero status code.

#### Scenario: Short APP_SECRET

- **WHEN** `APP_SECRET=short` is set
- **THEN** the process exits within 200 ms with
  `error: APP_SECRET must be at least 32 characters` and exit code 1.
