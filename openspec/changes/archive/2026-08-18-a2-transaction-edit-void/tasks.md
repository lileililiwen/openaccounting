## 1. Testing

- [x] 1.1 HTTP: `http_reverse_transaction_creates_pair`.
- [x] 1.2 HTTP: `http_edit_transaction_creates_pair`.
- [x] 1.3 HTTP: `http_reverse_viewer_403`.
- [x] 1.4 HTTP: `http_reverse_reversal_422`.
- [x] 1.5 Unit: `reversal_signed_amounts_are_negated` (`negate_postings` helper).
- [x] 1.6 DB: `db_reversal_link_row_updated`.

## 2. Implementation

- [x] 2.1 `migrations/0040_add_reversal_link.sql` (renumbered; the proposed `0032` was taken). Adds `transactions.reverses_id UUID REFERENCES transactions(id) ON DELETE SET NULL` + index.
- [x] 2.2 The existing `kind` CHECK constraint already allows `'reversing'`. No domain change required.
- [x] 2.3 `src/handlers/transactions_edit.rs` — `reverse` + `edit` handlers. Lock the original row with `SELECT … FOR UPDATE`; flip each posting's direction (DEBIT↔CREDIT) so the magnitude stays positive and the DB CHECK is honored. Reject `reversing` rows with `422 Unprocessable`.
- [x] 2.4 HTML templates: deferred. The POST endpoints are wired but the edit/reverse buttons in `transactions/show.html` are not added in this iteration (the spec only requires the routes). UI follow-up in a separate change.
- [x] 2.5 GL marker: deferred. Reports can be enriched in a follow-up change; the reversal rows are visible in the existing GL.
- [x] 2.6 `src/lib.rs` — `merge(handlers::transactions_edit::router())`.

## 3. Validation

- [x] 3.1 `openspec validate a2-transaction-edit-void` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 196 / 196 pass.
- [ ] 3.5 `openspec archive a2-transaction-edit-void`.
