# WASM In-Browser Demo

## Why

A web app can't be tried without installing. GnuCash is desktop; you
must download. A WASM-compiled version of the binary (or a slimmed
subset) that runs entirely in the browser with seeded data is a unique
positioning.

## What Changes

- New `wasm-demo/` workspace target that compiles a thin client
  (read-only seeded ledger) to wasm32-unknown-unknown.
- Hosts at `https://demo.openaccounting.dev`.
- Persists user changes to IndexedDB.
- The full server is unchanged; this is a separate crate that uses the
  same `domain` module.

## Capabilities

### New Capabilities

- `wasm-demo`: WASM in-browser read-only seeded demo.

## Impact

**New files:**
- `crates/wasm-demo/`.
- `static/demo/index.html`.
- `static/demo/seed.json`.
