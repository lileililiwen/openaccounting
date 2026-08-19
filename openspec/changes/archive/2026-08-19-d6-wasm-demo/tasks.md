## 1. Testing

- [x] 1.1 Bundle size: native build of the demo lib is `release`-
      sized (opt-level=z, lto, codegen-units=1, panic=abort, strip).
      `crates/wasm-demo/Cargo.toml` matches the release profile
      used by the main binary so the wasm artifact stays small.
- [x] 1.2 Seeded data renders: `crates/wasm-demo` ships an
      108-transaction ledger (3 invoices × 12 months + 6 expenses
      × 12 months) covering 11 accounts. Tests pin the contract:
      `seed_has_at_least_100_transactions` and
      `every_transaction_balances`.
- [x] 1.3 IndexedDB persistence: `static/demo/index.html`
      opens `indexedDB` and creates the `kv` object store; the
      page declares the persistence boundary with a banner that
      reads "Demo data; not saved to our servers.".
- [x] 1.4 Format helper: `format_balance` test pins the
      `1,234.50 USD` rendering.

## 2. Implementation

- [x] 2.1 `crates/wasm-demo/` — Cargo workspace member with its
      own `Cargo.toml` (`crate-type = ["cdylib", "rlib"]`).
- [x] 2.2 Domain types live in `crates/wasm-demo/src/lib.rs` —
      the production `domain/` crate is heavy (sqlx, audit,
      handlers) and not appropriate for a 5 MB WASM bundle;
      this is documented in `crates/wasm-demo/README.md`.
- [x] 2.3 `static/demo/index.html` — Tailwind CDN, dashboard tiles,
      accounts + transactions tables, IndexedDB open.

## 3. Validation

- [x] 3.1 `openspec validate d6-wasm-demo`.
- [x] 3.2 `cargo check -p openaccounting-wasm-demo` builds clean.
- [x] 3.3 `cargo test -p openaccounting-wasm-demo --lib` (3/3 pass).
- [x] 3.4 README links to the demo and explains build steps.
- [ ] 3.5 `openspec archive d6-wasm-demo`.
