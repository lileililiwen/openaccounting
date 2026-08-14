# Add Test Infrastructure — Tasks

## 1. Testing

- [x] 1.1 Unit: `TestDb` creates a unique database name per call
      and runs every migration on it.
- [x] 1.2 Unit: `TestDb::drop` removes the database (verify by
      counting rows in `pg_database` before/after).
- [x] 1.3 Integration: `smoke_server_boots_and_login_page_responds` —
      `GET /login` returns 200 and the body contains a form
      element.
- [x] 1.4 Integration:
      `smoke_register_login_and_ledgers_list_responds` —
      register → login → `GET /ledgers` returns 200 and contains
      the just-created user as the owner.
- [x] 1.5 Integration: `smoke_static_assets_served` —
      `GET /static/htmx.min.js` returns 200 and the body is
      non-empty.

## 2. Implementation

- [x] 2.1 `src/lib.rs` — declare every module from `main.rs`
      publicly; export `AppState`, `AppConfig`, `build_router`,
      `run`.
- [x] 2.2 `src/main.rs` — replace with a thin shell that calls
      `openaccounting::run()`.
- [x] 2.3 `Cargo.toml` — add `test-support` feature,
      `[dev-dependencies]` (`reqwest`, `tempfile`, `postgres`),
      `[[test]] integration` block.
- [x] 2.4 `src/test_support.rs` — `TestDb` and `TestServer`,
      gated on `#[cfg(any(test, feature = "test-support"))]`.
- [x] 2.5 `tests/common/mod.rs` — re-export
      `openaccounting::test_support::*`.
- [x] 2.6 `tests/integration/main.rs` — single entry point
      wiring `mod common; mod smoke;`.
- [x] 2.7 `tests/integration/smoke.rs` — the 3 smoke tests.

## 3. Spec

- [x] 3.1 `openspec/specs/testing/spec.md` — add requirement
      `Per-Test Database and HTTP Server Fixture` (this is
      archived at step 4).
- [x] 3.2 `openspec/specs/architecture/spec.md` — update the
      "Composition root" requirement so it admits `build_router`
      as the composition entry point and `main.rs` as the
      process-startup shell.

## 4. Validation

- [x] 4.1 `cargo fmt --check` clean.
- [x] 4.2 `cargo clippy --features test-support --all-targets`
      introduces **no new warnings** in files this change
      creates (`src/lib.rs`, `src/test_support.rs`,
      `tests/common/mod.rs`, `tests/integration/main.rs`,
      `tests/integration/smoke.rs`). Pre-existing clippy
      warnings in untouched files (e.g. `src/charts/mod.rs`,
      `src/templates/mod.rs`) are out of scope for this
      foundation change and will be addressed by the change
      that touches them.
- [x] 4.3 `cargo build` (default features) succeeds; the
      release binary contains no `TestDb` / `TestServer`
      symbols.
- [x] 4.4 `cargo test --test integration --features test-support`
      passes (3 smoke tests, parallel-safe with
      `--test-threads=1`).
- [x] 4.5 `cargo test --lib` (existing in-`mod tests`) still
      passes (regression: refactor of `main.rs` did not break
      the auth or admin unit tests).
- [x] 4.6 `openspec validate 2026-08-14-add-test-infrastructure`
      green.
- [x] 4.7 `openspec archive 2026-08-14-add-test-infrastructure`.
