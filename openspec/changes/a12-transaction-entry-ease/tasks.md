# Easier Transaction Entry

## 1. Testing

- [ ] 1.1 HTTP: `http_balancing_line_two_leg_balances` — JS-free proxy: post two lines, server accepts balanced entry.
- [ ] 1.2 HTTP: `http_simple_mode_submits_balanced_lines` — simple-mode fields map to two balanced postings.
- [ ] 1.3 HTTP: `http_simple_mode_transfer_directions` — transfer maps DEBIT to / CREDIT from.
- [ ] 1.4 HTTP: `http_inline_document_attached_on_create` — multipart create with a file → transaction + document row.
- [ ] 1.5 HTTP: `http_inline_create_without_document` — multipart create without file → transaction only.
- [ ] 1.6 HTTP: `http_inline_document_authorization` — a non-member cannot view the inline-attached document.
- [ ] 1.7 HTTP: `http_advanced_editor_unchanged` — existing urlencoded create path still works (regression).
- [ ] 1.8 JS unit (node, split.js/entry.js): balancing-line recompute, direction flip, promotion, status text.

## 2. Implementation

- [ ] 2.1 `static/js/entry.js`: balancing-line model (pin/infer, recompute, flip, promote) + status indicator.
- [ ] 2.2 `templates/partials/_posting_editor.html`: Simple/Advanced views sharing the posting table.
- [ ] 2.3 `templates/transactions/new.html`: render the editor partial + inline document input.
- [ ] 2.4 Simple-mode submit builder: inject hidden `lines[N]` inputs; preserve data on mode switch.
- [ ] 2.5 Create handler accepts multipart: parse urlencoded OR multipart; optional `files[]` written through storage and linked to the new transaction.
- [ ] 2.6 Reuse the existing document upload storage path; keep CSRF + body-limit working for both content types.
- [ ] 2.7 Show inline-attached documents on the transaction show page (existing renderer covers it).

## 3. Validation

- [ ] 3.1 `openspec validate a12-transaction-entry-ease`.
- [ ] 3.2 `cargo fmt --check`.
- [ ] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [ ] 3.4 `cargo test --features test-support --test integration -- transactions_split transactions_draft document_authorization` passes.
- [ ] 3.5 `openspec archive a12-transaction-entry-ease`.
