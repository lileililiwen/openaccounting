# Document Inbox & Multi-Leg Entry

## 1. Testing

- [ ] 1.1 HTTP: `http_inline_attach_on_create_kept` — a12 inline attach still works (regression, covered by `transactions_document`).
- [ ] 1.2 HTTP: `http_upload_unbound_document` — ledger-level upload → document row with NULL transaction_id + ledger anchor.
- [ ] 1.3 HTTP: `http_upload_unbound_non_writer_403`.
- [ ] 1.4 HTTP: `http_inbox_lists_unbound` — documents list shows unbound rows with a bind action.
- [ ] 1.5 HTTP: `http_bind_document_to_transaction` — bind sets transaction_id + audit row.
- [ ] 1.6 HTTP: `http_bind_by_creating_transaction` — bind flow creates a transaction and links the document.
- [ ] 1.7 HTTP: `http_bind_viewer_403`.
- [ ] 1.8 HTTP: `http_unbound_document_download_writer_ok` / `non_member_404`.
- [ ] 1.9 HTTP: `http_multileg_default` — the new-transaction form renders the multi-leg editor visible and Simple hidden (page content check).
- [ ] 1.10 JS (browser probe): default mode is advanced; simple↔advanced switch preserves data.

## 2. Implementation

- [ ] 2.1 `migrations/0047_add_unbound_documents.sql` — `ALTER documents ALTER transaction_id DROP NOT NULL; ADD ledger_id UUID NULL REFERENCES ledgers(id)`.
- [ ] 2.2 `src/domain/document.rs` — `transaction_id: Option<Uuid>`, `ledger_id: Option<Uuid>`.
- [ ] 2.3 `src/handlers/documents.rs` — ledger-level upload handler (`POST /ledgers/{id}/documents`) writing unbound rows.
- [ ] 2.4 Documents list: include unbound rows (`ledger_id` anchor) with a "Bind" action.
- [ ] 2.5 Bind handler + search form (`GET/POST /ledgers/{id}/documents/{doc_id}/bind`) + "create transaction" path.
- [ ] 2.6 Authorization: download branches on bound vs unbound; writers for unbound, members after bind.
- [ ] 2.7 `templates/documents/bind.html` + inbox list markup.
- [ ] 2.8 Multi-leg default: `entry.js` initial mode = advanced; `new.html` default `hidden` flipped.
- [ ] 2.9 Audit-log bind; keep inline attach + show-page upload unchanged.

## 3. Validation

- [ ] 3.1 `openspec validate a13-document-inbox`.
- [ ] 3.2 `cargo fmt --check`.
- [ ] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [ ] 3.4 `cargo test --features test-support --test integration -- transactions_document document_authorization transactions_split dashboard` passes.
- [ ] 3.5 `openspec archive a13-document-inbox`.
