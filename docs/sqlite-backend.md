# SQLite Backend (`d5-sqlite-option`)

OpenAccounting supports PostgreSQL (default) and SQLite
(opt-in). The two backends share the same schema migrations;
SQLite just needs a smaller subset of the runtime.

## When to use SQLite

SQLite is the right choice for:

- A **single-user desktop install**: `cargo run` on your laptop
  with no Docker / Postgres daemon.
- **Embedded demos**: the WASM demo crate ships with a seeded
  SQLite file (see `d6-wasm-demo`).
- **Tests**: the integration test suite can be re-pointed at
  SQLite to exercise the migration runner without a Postgres
  server.

SQLite is the wrong choice for:

- **Multi-writer production** — SQLite serializes writers at
  the file level, which does not scale beyond one process.
- **Anything that needs row-level locking** — SQLite locks the
  whole database for any write.
- **Anything that needs the missing Postgres features** (see
  "Unsupported features" below).

## Enabling

```toml
# Cargo.toml
openaccounting = { version = "0.1", features = ["db-sqlite"] }
```

The `db-sqlite` feature flips on sqlx's `sqlite` feature. The
default binary stays Postgres-only; SQLite is opt-in.

## Configuring

```sh
DATABASE_URL=sqlite:///path/to/openaccounting.db
```

The path is created if missing. Migrations run on first start.

## Unsupported Postgres features

The following features are NOT supported on SQLite:

| Feature | Why |
|---|---|
| `gen_random_uuid()` | SQLite needs `randomblob(16)` or app-side UUIDs; the `core` migration uses `gen_random_uuid()` because Postgres' `pgcrypto` extension is required. The SQLite migration variant (`migrations/0001_init.sqlite.sql`) substitutes `lower(hex(randomblob(16)))` (a 32-char hex string). |
| `MERGE` / `ON CONFLICT … DO UPDATE` | Postgres 15+ only. SQLite uses `ON CONFLICT (key) DO UPDATE SET …` instead. |
| `JSONB` columns | SQLite stores JSON as TEXT. The schema uses `TEXT` for JSON-shaped columns when the SQLite variant runs. |
| `uuid` type | SQLite has no native UUID type; UUIDs are stored as TEXT. |
| `numeric`/`decimal` precision | SQLite uses `REAL` (f64) for `NUMERIC`. The Rust layer still uses `rust_decimal::Decimal` and converts at the boundary. |
| Concurrent writers | See above. |

The migrations directory contains the canonical Postgres
version. The SQLite-flavoured runs use the same files
intentionally — they are written to be compatible with both
backends where possible. Where a migration is fundamentally
Postgres-only (e.g. `0019_add_reimbursement.sql` rewrites the
account subtype CHECK constraint in a Postgres-only way), the
binary refuses to start with a clear error.

## See also

- `openspec/changes/d5-sqlite-option/specs/sqlite-backend/spec.md`
- `src/config.rs` — `database_url_scheme` rejects unknown
  schemes.
- `openspec/specs/reports/spec.md` — basis semantics for reports.