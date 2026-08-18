# Apply Editor Role Enforcement to All Write Paths

## Why

`src/handlers/ledgers.rs:224` defines `ensure_owner` (owner-only) and
`ensure_editor` (owner OR editor), but `transactions::create`
(`src/handlers/transactions.rs:179`) calls `ensure_owner`. Editors cannot
create transactions, which contradicts the role defined in
`migrations/0008_add_ledger_sharing.sql`.

## What Changes

- Audit every write handler. Replace `ensure_owner` with
  `ensure_editor` where appropriate.
- Read-only handlers keep `ensure_access`.
- A new test suite (`tests/http/role_enforcement.rs`) covers owner/editor/
  viewer outcomes for each write path.

## Capabilities

### New Capabilities

- `role-enforcement`: Consistent editor/owner checks on write paths.

## Impact

**Modified files:**
- `src/handlers/transactions.rs` — `ensure_editor`.
- `src/handlers/accounts.rs` — `ensure_editor` for create/update.
- `src/handlers/invoices.rs`, `payments.rs`, `budgets.rs`, `taxes.rs`,
  `templates.rs`, `rules.rs`, `reimbursement.rs`, `fixed_assets.rs`,
  `inventory.rs`, `contacts.rs`, `documents.rs` — same.
- `tests/http/role_enforcement.rs` — new file.
