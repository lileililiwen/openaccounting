## 1. Testing

- [x] 1.1 Unit: `sniff_pdf_returns_application_pdf`.
- [x] 1.2 Unit: `sniff_html_renamed_pdf_rejected`.
- [x] 1.3 Unit: `sniff_csv_accepted_on_declaration`.
- [x] 1.4 HTTP: `http_upload_over_limit_returns_413`.
- [x] 1.5 HTTP: `http_upload_spoofed_type_rejected`.
- [x] 1.6 HTTP: `http_upload_quota_warn_logged_after_threshold`.

## 2. Implementation

- [x] 2.1 `Cargo.toml` — add `infer = "0.16"` and `tower-http` `limit` feature.
- [x] 2.2 `src/lib.rs` — install `RequestBodyLimitLayer` middleware (`tower_http::limit`) with `config.upload_max_bytes`. Over-cap requests get 413 immediately.
- [x] 2.3 `src/handlers/documents.rs` — call `upload::validate(declared, filename, buf)` after reading the multipart body; store the sniffed MIME (office / archive formats whose extension is in `EXTENSION_WINS` win over the declared header). Also added `upload::maybe_warn_quota()` after the DB insert.
- [x] 2.4 `src/handlers/reconciliation.rs::upload_csv` — same `upload::validate` call on the CSV body before parsing. (The proposal said "bank_feeds.rs" but the only ledger-scoped CSV upload route lives in `reconciliation.rs`; `bank_feeds.rs` uses provider webhooks, no multipart.)
- [x] 2.5 `src/config.rs` — `upload_max_bytes` field on `Config`, parsed from `UPLOAD_MAX_BYTES` env var, default 25 MiB; threaded through `AppConfig::with_upload_max_bytes()` and wired in `lib::run()`.

New module: `src/upload.rs` — the policy (CSV-accepted, plain-text-trusted, office-extension-wins), `UploadError`, `validate()`, `sniff()`, `maybe_warn_quota()`, plus 12 unit tests.

## 3. Validation

- [x] 3.1 `openspec validate s10-upload-validation` — passes.
- [x] 3.2 `cargo fmt --check` — clean.
- [x] 3.3 `cargo clippy --features test-support --tests` — no new warnings.
- [x] 3.4 `cargo test --features test-support --test integration` — 179 / 180 pass (1 pre-existing flaky test `document_ocr::http_apply_creates_reimbursement_line` fails under load but passes in isolation; not introduced by this change).
- [ ] 3.5 `openspec archive s10-upload-validation`.
