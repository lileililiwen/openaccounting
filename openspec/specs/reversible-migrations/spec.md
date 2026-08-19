# reversible-migrations Specification

## Purpose
TBD - created by archiving change o3-reversible-migrations. Update Purpose after archive.
## Requirements
### Requirement: Reversibility

MUST support `sqlx migrate revert` for every migration shipped in this repo; each migration MUST end with a `-- !DOWN` marker followed by a downward migration.

#### Scenario: Revert last

- **WHEN** `sqlx migrate revert` against the latest DB
- **THEN** the last migration is reversed; data is preserved where possible.

### Requirement: CI Proof

MUST include a CI job that applies all migrations, reverts all of them, and re-applies them.

#### Scenario: CI green

- **WHEN** the CI job runs against a fresh Postgres
- **THEN** the job exits 0.

### Requirement: New Migration Policy

MUST require all new migrations to be reversible; the OpenSpec change that introduces a migration MUST include a reversibility test.

#### Scenario: PR check

- **WHEN** a PR adds a non-reversible migration
- **THEN** the PR review checklist flags it.

### Requirement: Destructive Reversibility

MUST use `DROP IF EXISTS` and explicit `IF` guards so a revert that follows a partial failure does not leave the schema in a broken state.

#### Scenario: Idempotent revert

- **WHEN** revert after a partial forward
- **THEN** no SQL error.

