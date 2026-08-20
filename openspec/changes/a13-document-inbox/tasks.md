# Document Inbox & Multi-Leg Entry

## 1. Testing

- [x] 1.1 HTTP: `http_inline_attach_on_create_kept` — a12 inline attach still works (regression, covered by `transactions_document`).
- [x] 1.2 HTTP: `http_upload_unbound_document` — ledger-level upload → document row with NULL transaction_id + ledger anchor.
- [x] 1.3 HTTP: `http_upload_unbound_non_writer_403`.
- [x] 1.4 HTTP: `http_inbox_lists_unbound` — documents list shows unbound rows with a bind action.
- [x] 1.5 HTTP: `http_bind_document_to_transaction` — bind sets transaction_id + audit row.
- [x] 1.6 HTTP: `http_bind_by_creating_transaction` — bind flow creates a transaction and links the document.
- [x] 1.7 HTTP: `http_bind_viewer_403`.
- [x] 1.8 HTTP: `http_unbound_document_download_writer_ok` / `non_member_404`.
- [x] 1.9 HTTP: `http_multileg_default` — the new-transaction form renders the multi-leg editor visible and Simple hidden (page content check).
- [x] 1.10 JS (browser probe): default mode is advanced; simple↔advanced switch preserves data.

## 2. Implementation

- [x] 2.1 `migrations/0047_add_unbound_documents.sql` — `ALTER documents ALTER transaction_id DROP NOT NULL; ADD ledger_id UUID NULL REFERENCES ledgers(id)`.
- [x] 2.2 `src/domain/document.rs` — `transaction_id: Option<Uuid>`, `ledger_id: Option<Uuid>`.
- [x] 2.3 `src/handlers/documents.rs` — ledger-level upload handler (`POST /ledgers/{id}/documents`) writing unbound rows.
- [x] 2.4 Documents list: include unbound rows (`ledger_id` anchor) with a "Bind" action.
- [x] 2.5 Bind handler + search form (`GET/POST /ledgers/{id}/documents/{doc_id}/bind`) + "create transaction" path.
- [x] 2.6 Authorization: download branches on bound vs unbound; writers for unbound, members after bind.
- [x] 2.7 `templates/documents/bind.html` + inbox list markup.
- [x] 2.8 Multi-leg default: `entry.js` initial mode = advanced; `new.html` default `hidden` flipped.
- [x] 2.9 Audit-log bind; keep inline attach + show-page upload unchanged.

## 3. Validation

- [x] 3.1 `openspec validate a13-document-inbox`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support --tests` clean for changed files.
- [x] 3.4 `cargo test --features test-support --test integration -- transactions_document document_authorization transactions_split dashboard` passes.
- [x] 3.5 `openspec archive a13-document-inbox`.
