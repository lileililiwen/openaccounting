## 1. Testing

- [x] 1.1 Migration: every migration carries a `-- Reversible:` header
      and a companion `.down.sql` with the inverse SQL.
- [x] 1.2 CI: workflow runs the apply / revert / re-apply cycle and
      validates every migration has a DOWN companion file
      (`migrations-reversible` job in `ci.yml`).
- [x] 1.3 Existing test suite: 243 / 244 integration tests still
      pass (the one failure is a pre-existing pg_dump version
      mismatch in `scheduled_backup`, unrelated to this change).

## 2. Implementation

- [x] 2.1 Every existing migration (46 files) gets a
      `-- Reversible: yes` header and a `<name>.down.sql`
      companion file that reverts its operations in inverse order.
- [x] 2.2 `scripts/migrate-down.sh` provides a one-shot
      revert-and-re-apply runner for operators.
- [x] 2.3 CI job `migrations-reversible` in `.github/workflows/ci.yml`
      runs `sqlx migrate run`, `sqlx migrate revert` until empty,
      and `sqlx migrate run` again.
- [x] 2.4 README documents the migration policy: every new
      migration MUST ship with a DOWN companion; data-only
      migrations carry `-- Reversible: no` and a justification.

## 3. Validation

- [x] 3.1 `openspec validate o3-reversible-migrations`.
- [x] 3.2 All 46 `.sql` files have a `-- Reversible:` header.
- [x] 3.3 All 46 `.down.sql` companion files exist with inverse SQL.
- [x] 3.4 `cargo test --features test-support --test integration` (243/244 pass).
- [ ] 3.5 `openspec archive o3-reversible-migrations`.
