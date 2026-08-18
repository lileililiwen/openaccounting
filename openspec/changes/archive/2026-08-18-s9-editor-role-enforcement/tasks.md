## 1. Testing

- [x] 1.1 HTTP: `http_owner_can_create_transaction`.
- [x] 1.2 HTTP: `http_editor_can_create_transaction`.
- [x] 1.3 HTTP: `http_viewer_cannot_create_transaction` — 403.
- [x] 1.4 HTTP: same matrix for accounts, invoices, budgets, contacts.
- [x] 1.5 HTTP: `http_editor_cannot_invite_member`.
- [x] 1.6 HTTP: `http_viewer_cannot_delete_budget` (account delete is not a feature; budgets/delete exercises the same `ensure_writer` path — 403).

## 2. Implementation

- [x] 2.1 Swap `ensure_owner` → `ensure_writer` (returns 403 for viewer; owner OR editor) in `transactions.rs`, `accounts.rs`, `invoices.rs`, `payments.rs`, `budgets.rs`, `taxes.rs`, `templates.rs`, `rules.rs`, `reimbursement.rs`, `fixed_assets.rs`, `inventory.rs`, `contacts.rs`, `documents.rs`. Removed unused `ensure_editor`; added `ensure_owner_strict` (returns 403 for non-owner).
- [x] 2.2 Sharing handlers (`sharing::page`, `sharing::invite`, `sharing::remove_member`) use `ensure_owner_strict` so editors get 403 (per the spec scenario "editor tries to invite → 403"), not the silent 404 from `ensure_owner`.
- [x] 2.3 Audit ledger-deletion path is owner-only — there is no HTTP ledger-deletion route in `lib.rs` yet; any future route MUST use `ensure_owner_strict`. Recorded here as a constraint for future changes.

## 3. Validation

- [x] 3.1 `openspec validate s9-editor-role-enforcement` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings introduced.
- [x] 3.4 `cargo test --features test-support --test integration role_enforcement` — 15/15 pass. Full integration suite: 173 passed, 1 pre-existing failure (document_ocr::http_apply_creates_reimbursement_line) unrelated to this change.
- [ ] 3.5 `openspec archive s9-editor-role-enforcement`.
