# 1. Testing

- [x] 1.1 HTTP: invoice create/mark-paid/void round-trip via API; UI reflects change.
- [x] 1.2 HTTP: viewer token 403 on each new write endpoint (matrix test).
- [x] 1.3 Integration: idempotent retry across simulated restart (drop cache, re-send key) → replay, no duplicate.
- [x] 1.4 HTTP: same key + different body → 422; nothing executed.
- [x] 1.5 Property: cursor walk over 500 random rows visits each exactly once; forged cursor → 400.
- [x] 1.6 Integration: 121st request in a window → 429 with Retry-After; second token unaffected.
- [x] 1.7 HTTP: cash-flow API equals HTML report figures for fixture ledger.
- [x] 1.8 HTTP: document upload via API enforces size/MIME caps and appears in UI document list.

# 2. Implementation

- [x] 2.1 Migration `api_idempotency`; table-backed store replacing HashMap.
- [x] 2.2 Pagination helper + signed cursors; retrofit existing lists.
- [x] 2.3 Per-token sliding-window middleware.
- [x] 2.4 `src/api/invoices.rs`, `payments.rs`, `contacts.rs`.
- [x] 2.5 `src/api/documents.rs` (multipart through `src/upload.rs` validation).
- [x] 2.6 `src/api/budgets.rs`, `bank_feeds.rs` (read-only).
- [x] 2.7 Cash-flow endpoint delegates to full report service.

# 3. Validation

- [x] 3.1 `openspec validate api-expansion`.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets --features test-support`.
- [x] 3.3 `cargo test --features test-support`.
