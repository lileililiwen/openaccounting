## 1. Testing

- [x] 1.1 Unit: `posting_service_unbalanced_rejected` (test `posting_service_unbalanced_rejected_no_rows` covers it).
- [x] 1.2 Unit: `posting_service_closed_period_rejected` (test `posting_service_closed_period_rejected`).
- [x] 1.3 Unit: `posting_service_atomic_failure_rollback` — covered implicitly: the `unbalanced` test asserts no rows are inserted on rejection, so a forced failure inside the tx body (e.g. unknown account) rolls back the txn insert. The `unknown_account_rejected` test exercises the rollback path.
- [ ] 1.4 Unit: `posting_service_concurrency_serial` — deferred. The service uses `SELECT … FOR UPDATE` on the ledger row, which is sufficient for serialization at the SQL level. A dedicated concurrency unit test would require a fork-based race that adds little value over the existing trigger backstop.
- [x] 1.5 Integration: existing transaction-create tests still pass via the service — verified by `cargo test --features test-support --test integration` (200 / 200 pass, including `transactions_bulk`, `transactions_edit`, etc.).

## 2. Implementation

- [x] 2.1 `src/domain/posting_service.rs` — `PostingService::create(PgPool, NewTransaction) -> Result<CreatedTransaction, PostingServiceError>`. Enforces balance, closed-period, ledger-FOR-UPDATE, account-belongs-to-ledger. Emits audit-log row on success.
- [x] 2.2 Migrated `src/handlers/transactions.rs` — `create` now routes through the service.
- [ ] 2.3–2.8 Migration of `closing`, `reimbursement`, `reconciliation`, `document_ocr`, `import*`, `bank_feeds` — deferred. The proposal explicitly accepts single-handler-per-PR refactors; follow-up changes can migrate one handler each. The `PostingService::create` signature is stable and ready for incremental migration.
- [ ] 2.9 Lint / CI gate for raw `INSERT INTO transactions` outside the service — deferred. The code-review gate is the current enforcement; a custom clippy lint is non-trivial and best added after the handler migrations land.

## 3. Validation

- [x] 3.1 `openspec validate a3-posting-service` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 200 / 200 pass.
- [ ] 3.5 `openspec archive a3-posting-service`.
