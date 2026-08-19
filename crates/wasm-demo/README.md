# WASM In-Browser Demo (`d6-wasm-demo`)

A read-only demo of OpenAccounting that runs entirely in the
browser. There is no server. The seeded ledger ships as a Rust
data structure compiled to WASM.

## Build

The demo lives in `crates/wasm-demo`. To compile:

```sh
rustup target add wasm32-unknown-unknown
cargo build -p openaccounting-wasm-demo \
    --target wasm32-unknown-unknown --release
wasm-bindgen \
    --target web --out-dir static/demo \
    target/wasm32-unknown-unknown/release/openaccounting_wasm_demo.wasm
```

Then serve `static/demo/` from any static HTTP server. The
demo runs offline once the WASM bundle is cached.

## What it shows

A realistic 110-transaction ledger for "Acme Coffee Roasters"
across 10 months:

- 30 invoices totaling $114,300 (split across 3 customers)
- 60 expenses totaling $106,800 (rent, SaaS, office, marketing,
  travel)
- 11 accounts in the seeded chart of accounts

Each transaction is balanced (Σ debits = Σ credits), matching
the production invariant. The dashboard tiles compute the
total movement and transaction count on the client.

## Persistence

The demo does NOT write to the server. Edits stay in the
browser's IndexedDB; a banner on every page reminds the user.

## See also

- `openspec/changes/d6-wasm-demo/specs/wasm-demo/spec.md`
- `crates/wasm-demo/src/lib.rs` — the demo crate.
- `static/demo/index.html` — the entry page.