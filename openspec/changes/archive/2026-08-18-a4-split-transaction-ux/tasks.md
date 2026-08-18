## 1. Testing

- [x] 1.1 HTTP: `http_create_three_leg_split_succeeds`.
- [x] 1.2 HTTP: `http_create_two_leg_with_auto_balance_value`.
- [x] 1.3 HTTP: `http_single_row_rejected` (returns the form with an error).
- [ ] 1.4 Manual screenshot — deferred. UI is functional; visual verification is left to a follow-up.

## 2. Implementation

- [x] 2.1 `templates/transactions/new.html` — added a "⚖ Split" button + N-input next to the existing "+ Add line". A live "✓ balanced / net X" indicator sits between the heading and the buttons.
- [x] 2.2 `static/js/split.js` — vanilla JS, no build step. Clicking Split inserts (N − existing) blank rows, marks the last row's amount + direction for auto-fill, recomputes the live balance on every input event, and toggles the last row's direction when the user types an unbalanced amount.
- [x] 2.3 `src/handlers/transactions.rs::create` — already accepts any number of rows; now routes through `PostingService::create` (a3). The a3 service disables `trg_posting_balance` for the duration of the write tx so multi-leg inserts (where the running sum doesn't balance until the last leg) succeed.

## 3. Validation

- [x] 3.1 `openspec validate a4-split-transaction-ux` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 202 / 203 pass (1 pre-existing flaky `ocr_feedback::http_ocr_corpus_export_admin_only` passes in isolation; unrelated).
- [ ] 3.5 `openspec archive a4-split-transaction-ux`.
