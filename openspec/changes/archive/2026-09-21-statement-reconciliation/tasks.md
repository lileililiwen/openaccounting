## 1. Testing

- [x] 1.1 Unit: difference computation over opening + cleared lines matches statement balance cases.
- [x] 1.2 Unit: CAMT.053 fixture parses to normalized lines with correct signs.
- [x] 1.3 Unit: QBO fixture parses via extended OFX path.
- [x] 1.4 Integration: finish with nonzero difference leaves session open; zero closes and locks.
- [x] 1.5 Property: random cleared subsets always compute difference consistently with Decimal exactness.
- [x] 1.6 HTTP: finish on unbalanced session returns 409 with difference in body.
- [x] 1.7 HTTP: unreconcile without reason returns 400; with reason reopens and audits.
- [x] 1.8 E2E: import statement → clear lines → finish at zero → verify lock blocks unclear.

## 2. Implementation

- [x] 2.1 Migration: `rec_sessions` and `rec_lines` tables with indexes.
- [x] 2.2 Domain: session open/finish/unreconcile logic with zero-gate and audit writes.
- [x] 2.3 Import: CAMT.053 and QBO parsers with fixture-backed error messages.
- [x] 2.4 Rules: surface rule suggestions as cleared candidates requiring confirm.
- [x] 2.5 Bank feeds: document EU aggregator provider shape behind Provider trait.
- [x] 2.6 Templates: session page with difference indicator, lock banner, unreconcile form.

## 3. Validation

- [x] 3.1 `openspec validate statement-reconciliation` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including new rec tests.
