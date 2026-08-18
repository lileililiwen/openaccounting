## 1. Testing

- [x] 1.1 HTTP: `http_inter_ledger_transfer_creates_two_txns` — one txn per ledger + one `inter_ledger_transfers` row.
- [ ] 1.2 HTTP: `http_inter_ledger_transfer_with_currency` — deferred. Both ledgers in the test bootstrap use USD; the handler already reads each ledger's `base_currency` for the link row. A cross-currency round-trip is a follow-up.
- [ ] 1.3 HTTP: `http_inter_ledger_transfer_with_fee` — deferred. The handler records `fee_amount` on the link row but does not post a fee entry (requires a clearing-account model). Documented.
- [x] 1.4 HTTP: `http_inter_ledger_transfer_unauthorized_403` — non-owner non-editor gets 403/404.
- [x] 1.5 HTTP: `http_inter_ledger_transfer_rejects_same_ledger` — from == to returns 400.
- [x] 1.6 HTTP: `http_inter_entity_report_renders` — the report lists the transfer for both ledgers.

## 2. Implementation

- [x] 2.1 `migrations/0043_add_inter_ledger_transfers.sql` (renumbered; proposed 0035 was already used). `inter_ledger_transfers(id, from_ledger_id, to_ledger_id, from_account_id, to_account_id, amount, currency, from_txn_id, to_txn_id, fee_amount, fee_account_id, description)` + `CHECK (from_ledger_id <> to_ledger_id)` + `CHECK (from_txn_id <> to_txn_id)`.
- [x] 2.2 `src/handlers/transfers.rs` — `create` (POST /transfers/inter-ledger) inserts both transactions on a single connection inside one tx; `reverse` (POST /transfers/inter-ledger/reverse) inserts reversals via `transactions_edit::insert_reversal`; `new_page` is a placeholder until the UI ships.
- [x] 2.3 `src/reports/inter_entity.rs` — JSON `GET /ledgers/{id}/reports/inter-entity`.
- [ ] 2.4 `templates/transfers/new.html` — deferred. The form posts the right field names; the Askama template is a follow-up.
- [x] 2.5 `src/lib.rs` — `/transfers/inter-ledger` + `/transfers/inter-ledger/reverse` + `/ledgers/{id}/reports/inter-entity` wired.

## 3. Validation

- [x] 3.1 `openspec validate a7-inter-ledger-transfers` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 219 / 219 pass (one pre-existing flaky pass on second run).
- [ ] 3.5 `openspec archive a7-inter-ledger-transfers`.
