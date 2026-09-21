## 1. Testing

- [x] 1.1 Unit: idempotency store returns original response hash for replayed keys within TTL.
- [x] 1.2 Integration: route-inventory test enumerates all /api/v1 routes and asserts OpenAPI coverage.
- [x] 1.3 Integration: unlisted event emission fails the catalog test.
- [x] 1.4 HTTP: POST with Idempotency-Key twice creates one resource with identical responses.
- [x] 1.5 HTTP: incoming signed event intake enqueues the mapped job.
- [x] 1.6 HTTP: rule CRUD is owner-only; editors get 403.
- [x] 1.7 E2E: create rule → trigger event → verify webhook delivery and job audit entry.

## 2. Implementation

- [x] 2.1 OpenAPI YAML plus serving route and inventory test.
- [x] 2.2 Complete per-resource CRUD gaps in src/api routers.
- [x] 2.3 Idempotency middleware backed by existing idempotency table with TTL prune.
- [x] 2.4 Incoming event intake with signature verification and job mapping.
- [x] 2.5 Rule builder UI and execution on scheduler queue with rate limits.
- [x] 2.6 Docs: docs/api-reference.md and event catalog with payload examples.

## 3. Validation

- [x] 3.1 `openspec validate openapi-sdk` passes.
- [x] 3.2 `cargo fmt --check && cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.3 `cargo test --features test-support` passes including inventory and idempotency tests.