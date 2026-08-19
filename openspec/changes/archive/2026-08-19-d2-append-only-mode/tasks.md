## 1. Testing

- [x] 1.1 HTTP: `http_append_only_toggle_owner_succeeds`.
- [x] 1.2 HTTP: `http_append_only_blocks_edit_returns_422`.
- [x] 1.3 HTTP: `http_append_only_allows_reverse`.
- [x] 1.4 HTTP: `http_append_only_blocks_raw_account_update`.
- [x] 1.5 HTTP: `http_append_only_editor_cannot_disable`.
- [x] 1.6 HTTP: `http_append_only_toggle_editor_forbidden`.

## 2. Implementation

- [x] 2.1 `migrations/0044_add_ledger_append_only.sql`.
- [x] 2.2 `Ledger` struct gains `append_only: bool` + new SELECT/INSERT lists.
- [x] 2.3 `transactions_edit` checks `append_only`; returns 422.
- [x] 2.4 DB trigger blocks UPDATE on accounts in append-only mode.
- [x] 2.5 DB trigger blocks UPDATE/DELETE on transactions in append-only mode.
- [x] 2.6 `toggle_append_only` handler + route + audit-log call.
- [x] 2.7 `LedgerShow` template renders enable / disable controls.
- [x] 2.8 `LedgerDto` carries the flag for the REST API.

## 3. Validation

- [x] 3.1 `openspec validate d2-append-only-mode`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [x] 3.4 `cargo test --features test-support --test integration -- append_only` (6/6 pass).
- [ ] 3.5 `openspec archive d2-append-only-mode`.
