# sqlite-backend Specification (delta)

## ADDED Requirements

### Requirement: DATABASE_URL Selection

MUST accept `sqlite://<path>` and run on SQLite; the path is created if missing.

#### Scenario: First run

- **WHEN** the user sets DATABASE_URL=sqlite://./data/oa.db
- **THEN** the file is created; migrations run; the app starts.

### Requirement: Same Surface

MUST expose identical HTTP behavior on SQLite; tests MUST pass on both.

#### Scenario: Same routes

- **WHEN** the user runs the same e2e tests
- **THEN** they pass on Postgres and SQLite.

### Requirement: Document Differences

MUST document which PG-specific features are NOT supported on SQLite (e.g. concurrent writers, `pg_dump`-style backups).

#### Scenario: Doc

- **WHEN** the README
- **THEN** the differences are listed.

### Requirement: No Silent Fallback

MUST refuse to start if DATABASE_URL is neither Postgres nor SQLite.

#### Scenario: Unknown scheme

- **WHEN** DATABASE_URL=mysql://…
- **THEN** startup fails with a clear error.
