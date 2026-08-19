## 1. Testing

- [x] 1.1 Unit: `filesystem_round_trip`.
- [x] 1.2 Unit: `concurrent_uploads_succeed`.
- [x] 1.3 Integration: `http_download_returns_bytes_via_signed_url_when_s3`
      (FS backend returns `None`; signed-URL behaviour is exercised
      against the S3 impl, which the test file documents).
- [x] 1.4 HTTP: existing document-authorization + upload-validation
      tests still pass (the read path uses the new trait API).

## 2. Implementation

- [x] 2.1 `src/storage/mod.rs` — `Storage` trait, `StorageKey`, `SharedStorage`.
- [x] 2.2 `src/storage/filesystem.rs` — implements the trait.
- [x] 2.3 `src/storage/s3.rs` — S3 backend via `aws-sdk-s3`.
- [x] 2.4 `src/lib.rs::AppState` — `Arc<dyn Storage>`.
- [x] 2.5 `Cargo.toml` — `storage-s3` feature gates the AWS SDK crates.
- [x] 2.6 `docs/release-verification.md` already documents the
      feature flag (no extra changes; the same env-var convention
      applies).
- [x] 2.7 Caller updates: `documents.rs`, `document_ocr.rs`,
      `health.rs` now go through the trait API.
- [x] 2.8 Backwards-compat: `stored_filename` in the DB stores
      the backend-relative path (the new upload handler writes
      `{txn_id}/{uuid}-{name}`; existing tests are updated to
      match the new layout).

## 3. Validation

- [x] 3.1 `openspec validate o6-s3-storage`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --features test-support` clean for changed files.
- [x] 3.4 `cargo test --features test-support --test integration -- s3_store upload_validation document_authorization` (15/15 pass).
- [x] 3.5 `cargo check --features test-support,storage-s3` compiles.
- [ ] 3.6 `openspec archive o6-s3-storage`.
