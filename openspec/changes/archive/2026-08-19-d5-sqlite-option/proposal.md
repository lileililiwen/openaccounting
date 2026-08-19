# Optional SQLite Backend

## Why

Postgres is a heavy operational requirement for a freelancer wanting
to track expenses. GnuCash uses SQLite; Beancount uses flat files. Many
potential users would adopt OpenAccounting if they could `cargo run` and
get a working ledger without any DB setup.

## What Changes

- Add a `database_url` parser that accepts `sqlite://path/to/db`.
- Use `sqlx` with the `sqlite` feature.
- Re-validate every migration for SQLite compatibility (no PG-specific
  features).
- Re-test every domain helper for SQLite.
- Default stays Postgres; SQLite is opt-in via DATABASE_URL.

## Capabilities

### New Capabilities

- `sqlite-backend`: Optional SQLite storage.

## Impact

**Modified files:**
- `Cargo.toml` — `sqlx` sqlite feature.
- `migrations/*.sql` — must be SQLite-compatible (separate review per
  migration).
- `README.md` — `DATABASE_URL=sqlite://…`.
