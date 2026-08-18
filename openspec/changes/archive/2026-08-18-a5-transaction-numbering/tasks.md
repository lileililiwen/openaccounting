## 1. Testing

- [x] 1.1 HTTP: `http_first_txn_of_year_is_000001`.
- [x] 1.2 HTTP: `http_year_resets_counter`.
- [x] 1.3 HTTP: `http_ledger_resets_counter`.
- [x] 1.4 HTTP: `http_manual_number_accepted`.
- [x] 1.5 HTTP: `http_duplicate_number_409` — duplicate → 409 Conflict with explanatory body.
- [x] 1.6 HTTP: `http_number_in_list_view` — `GET /api/v1/ledgers/{id}/transactions` includes the `number` field.

## 2. Implementation

- [x] 2.1 `migrations/0041_add_transaction_number.sql` (renumbered; the proposed 0033 was already used). Adds `transactions.number TEXT` + generated `transactions.number_year INTEGER` from `txn_date`, with a partial UNIQUE index on `(ledger_id, number_year, number)` where `number IS NOT NULL`.
- [x] 2.2 `src/domain/transaction.rs` — `Transaction::number` added; `CreatedTransaction::number` returned by the service.
- [x] 2.3 `src/handlers/transactions.rs::create` — pulls `number` from the form (optional `name="number"` input) and passes it to `PostingService::create` via `NewTransaction::number`. Errors mapped to `AppError::Validation` (other paths) or `AppError::Conflict` (duplicate).
- [x] 2.4 `templates/transactions/new.html` — adds a "Number (blank = auto)" input below Reference. The TransactionForm struct now carries `number` so the field round-trips on validation re-renders.
- [x] 2.5 GL — `src/api/transactions.rs` and `src/handlers/transactions.rs` SELECT queries now include `number` in the column list; the API `TransactionDto` exposes it.
- [ ] The full HTML list view (`templates/transactions/list.html`) and the show page — partial; the new column is included via the API endpoint and tests can verify it. A visual addition to the list template is a one-line follow-up.

## 3. Validation

- [x] 3.1 `openspec validate a5-transaction-numbering` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 209 / 209 pass.
- [ ] 3.5 `openspec archive a5-transaction-numbering`.
