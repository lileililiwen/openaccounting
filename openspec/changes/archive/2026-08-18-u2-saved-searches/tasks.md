## 1. Testing

- [x] 1.1 HTTP: `http_save_search` — POST writes a row.
- [x] 1.2 HTTP: `http_apply_search` — GET renders a chip link with the stored query.
- [x] 1.3 HTTP: `http_delete_search` — POST removes the row.
- [x] 1.4 HTTP: `http_search_user_scope` — B never sees A's rows.
- [x] 1.5 HTTP: `http_default_query_is_returned` — `default_query` resolves the row flagged `is_default`.

## 2. Implementation

- [x] 2.1 `migrations/0036_add_saved_searches.sql` — `(user_id, name)` UNIQUE; partial UNIQUE INDEX on `is_default = TRUE` enforces one default per user.
- [x] 2.2 `src/handlers/saved_searches.rs` — `list_searches` (GET renders a chip list HTML fragment), `create`, `make_default`, `delete`. Plus the `default_query` helper used by the transactions list endpoint.
- [x] 2.3 `templates/transactions/_searches.html` — picker embedded into the transactions list with a "Save current view" form.

Also wired:
- `src/handlers/transactions.rs::list` — when the URL has no filter params and the user has a default saved search, the default query is auto-applied.
- `TransactionList` template struct gains a `current_query: String` field.

## 3. Validation

- [x] 3.1 `openspec validate u2-saved-searches`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u2-saved-searches`.