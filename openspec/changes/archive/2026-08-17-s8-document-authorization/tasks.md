## 1. Testing

- [x] 1.1 HTTP: `http_doc_owner_can_download`.
- [x] 1.2 HTTP: `http_doc_editor_can_download`.
- [x] 1.3 HTTP: `http_doc_viewer_can_download`.
- [x] 1.4 HTTP: `http_doc_cross_ledger_returns_404`.
- [x] 1.5 HTTP: `http_doc_cross_user_returns_404`.
- [x] 1.6 HTTP: `http_doc_cross_ledger_delete_returns_404`.

## 2. Implementation

- [x] 2.1 `src/handlers/documents.rs::ensure_doc_access`.
- [x] 2.2 Apply in download, delete, ocr handlers.
- [x] 2.3 Same helper reused in `document_ocr.rs`.

## 3. Validation

- [x] 3.1 `openspec validate s8-document-authorization`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive s8-document-authorization`.