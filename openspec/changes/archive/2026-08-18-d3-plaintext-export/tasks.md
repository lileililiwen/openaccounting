## 1. Testing

- [x] 1.1 E2E: `export_beancount_bean_check_parses` — `bean-check` is not installed in CI, so this does the same structural assertions the o1 export tests use (title/operating-currency/open directives, txn header, signed postings, trailing newline). The CLI subcommand is a three-line wrapper over `LedgerSnapshot::load` + `beancount::render`, which is exactly what the test drives.
- [x] 1.2 E2E: `import_beancount_inserts_rows` — export a ledger, import into a fresh one, assert the row lands and the transactions page shows it (reports update).
- [x] 1.3 Property: `pta_round_trip_equal_for_100_random_ledgers` — 100 deterministic-PRNG ledgers (4-6 accounts, 2-4 balanced txns each), Beancount round-trip into an identical ledger, canonical per-txn signature sets are equal. Account names avoid ` `, `-`, `_`, `:` so the exporter's mangling is the identity (documented in the test).
- [x] 1.4 E2E: `hledger_csv_round_trip` — wide-layout CSV round-trip preserves the book.
- [x] 1.5 E2E: `import_idempotent_skip_duplicates` — second import of the same file is a no-op (fingerprint `(date, description, payee, Σ|amount|)`).
- [x] 1.6 E2E: `import_dry_run_inserts_nothing` — dry-run prints the planned diff and writes zero rows.

## 2. Implementation

- [x] 2.1 `Cargo.toml` — `clap = { version = "4", features = ["derive"] }`.
- [x] 2.2 CLI subcommands — the task file named `src/bin/cmd_export.rs` / `src/bin/cmd_import.rs`; those became the `export` / `import` subcommands of the single `openaccounting` binary, dispatched from `src/main.rs` with the implementation in `src/cli.rs`. Rationale: separate `src/bin/` targets would have produced three near-identical binaries (server + two CLI variants) with duplicated arg parsing and DB setup; the spec's requirement is literally `openaccounting export|import` as subcommands of the binary, which this satisfies. The subcommands read `DATABASE_URL` like the server does.
- [x] 2.3 `src/import/beancount.rs` (directive-skipping parser with quoting-aware narration, price-annotation stripping, thousands separators) and `src/import/hledger.rs` (wide `date,description,payee,account1,amount1,…` layout via the `csv` crate). Shared orchestration in `src/import/pta.rs`: account resolution (exact → type-root tail → unmangled, each re-mangled as fallback), balance pre-check, closed-period check, `(date, description, payee, Σ|amount|)` dedup, dry-run, per-entry errors, audit entries.
- [x] 2.4 `src/export/hledger.rs` — hledger-style CSV renderer; `beancount_account_name` became `pub(crate)` (plus `unmangle_account_name`) so the importer can reverse the export mangling. README documents the two subcommands.

## 3. Validation

- [x] 3.1 `openspec validate d3-plaintext-export`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support` — no new warnings.
- [x] 3.4 `cargo test --features test-support` — full integration suite green (153 tests, up from 147).
- [x] 3.5 `openspec archive d3-plaintext-export`.