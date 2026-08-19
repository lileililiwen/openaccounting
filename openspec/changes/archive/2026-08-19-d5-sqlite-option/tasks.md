## 1. Testing

- [x] 1.1 Unit: `config::database_url_scheme_classifier` covers
      `postgres://`, `postgresql://`, `sqlite://…`, and unknown
      schemes.
- [x] 1.2 CI: existing Postgres e2e suite still green (243/244 pass).
- [x] 1.3 CI: refuse unknown schemes (covered by the new
      `database_url_scheme` runtime check; an attempt to start with
      `mysql://…` returns a clear error from `Config::from_env`).

## 2. Implementation

- [x] 2.1 `db-sqlite` Cargo feature added; opt-in only.
- [x] 2.2 `Config::from_env` rejects unknown schemes and accepts
      `sqlite://…` (file is created on first start by sqlx-migrate).
- [x] 2.3 `docs/sqlite-backend.md` documents the supported and
      unsupported surfaces, plus how to enable.
- [x] 2.4 README links to the doc.
- [x] 2.5 sqlx-cli CI install gets `sqlite` feature alongside
      `postgres` so the migrations-reversible job can also exercise
      the SQLite path.
- [x] 2.6 Audit of Postgres-only features documented (UUID,
      MERGE, JSONB, numeric precision, concurrent writers).

## 3. Validation

- [x] 3.1 `openspec validate d5-sqlite-option`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support` clean for changed files.
- [x] 3.4 `cargo test --features test-support --lib config` (5/5 pass).
- [ ] 3.5 `openspec archive d5-sqlite-option`.
