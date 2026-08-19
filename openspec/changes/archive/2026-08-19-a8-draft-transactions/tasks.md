## 1. Testing

- [x] 1.1 HTTP: `http_save_as_draft_persists_with_draft_kind`.
- [x] 1.2 HTTP: `http_draft_excluded_from_reports`.
- [x] 1.3 HTTP: `http_promote_draft_included_in_reports`.
- [x] 1.4 HTTP: `http_discard_draft_no_reversal`.
- [x] 1.5 HTTP: `http_draft_in_closed_period`.
- [x] 1.6 HTTP: `http_drafts_page_lists_all_drafts`.

## 2. Implementation

- [x] 2.1 `migrations/0045_add_draft_kind.sql` (CHECK widened).
- [x] 2.2 `PostingService` — `create_draft`, `post_draft`, `delete_draft`.
- [x] 2.3 `src/handlers/transactions_draft.rs` (list, post, discard).
- [x] 2.4 `templates/transactions/drafts.html` + `src/templates/transactions_draft.rs`.
- [x] 2.5 `src/lib.rs` — routes wired via `merge(...)`.
- [x] 2.6 Report queries (trial balance, balance sheet, P&L, cash flow, GL, cash-flow forecast, realized gains) filter `kind != 'draft'`.
- [x] 2.7 New-transaction form gets a "Save as draft" button.

## 3. Validation

- [x] 3.1 `openspec validate a8-draft-transactions`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support` clean for new files.
- [x] 3.4 `cargo test --features test-support --test integration -- transactions_draft` (6/6 pass).
- [ ] 3.5 `openspec archive a8-draft-transactions`.
