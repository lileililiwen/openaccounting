## 1. Testing

- [x] 1.1 Unit: dimension-filtered TB sums match unfiltered totals partitioned by dimension.
- [x] 1.2 Unit: FIFO vs average valuation on fixture movements produce disclosed distinct values.
- [x] 1.3 Unit: declining-balance schedule matches fixture table to the cent.
- [x] 1.4 Integration: recurring template generates preview drafts without posting.
- [x] 1.5 Property: random inventory movement sequences keep quantity and value consistent per method.
- [x] 1.6 HTTP: dimension slice params filter P&L; Unassigned bucket holds untagged postings.
- [x] 1.7 HTTP: scheduler double-fire posts once per template period.
- [x] 1.8 HTTP: FX override without reason fails 400; with reason audits and applies.

## 2. Implementation

- [x] 2.1 Migration: dimension tables, posting dimension columns, recurring templates, FX override audit.
- [x] 2.2 Domain: dimension-aware posting validation preserving balance invariant.
- [x] 2.3 Recurring engine: template CRUD, preview drafts, idempotent scheduler posting, skip/pause.
- [x] 2.4 Inventory: FIFO and weighted-average with method guard and disclosure.
- [x] 2.5 Assets: straight-line and declining-balance with disposal gain/loss.
- [x] 2.6 FX: override audit trail wired into lookup and revaluation.
- [x] 2.7 Reports and templates: dimension filters, method disclosure lines.

## 3. Validation

- [x] 3.1 `openspec validate accounting-dimensions` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including valuation tests.
