## 1. Testing

- [x] 1.1 HTTP: `http_empty_transactions_renders_partial`.
- [x] 1.2 HTTP: `http_empty_accounts_renders_partial`.

## 2. Implementation

- [x] 2.1 `templates/partials/_empty.html` per resource (`_empty_transactions.html`, `_empty_accounts.html`, `_empty_invoices.html`, `_empty_budgets.html`).
- [x] 2.2 Four SVG files in `static/img/empty/` (transactions, accounts, invoices, budgets).
- [x] 2.3 Update each list handler / template (transactions, accounts, invoices, budgets).

## 3. Validation

- [x] 3.1 `openspec validate u9-empty-states`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u9-empty-states`.