# Receipt OCR — Tasks

## 1. Testing

- [x] 1.1 Unit: `parse_amount` for `¥1,234.56`, `12.34 USD`,
      `Total: 12.34`.
- [x] 1.2 Unit: `parse_date` for `Aug 14, 2026`, `2026-08-14`.
- [x] 1.3 Unit: `extract_merchant` returns the first
      non-empty line of length 4-40.
- [x] 1.4 Property: `prop_amount_parser_handles_currency_symbols`
      for 1000 random combos.
- [x] 1.5 Integration: `http_upload_image_kicks_off_ocr` —
      poll until result.
- [x] 1.6 Integration:
      `http_apply_creates_reimbursement_line`.
- [x] 1.7 Integration: `http_upload_with_ocr_false_skips_job`.
- [x] 1.8 Integration:
      `http_pdf_without_pdftoppm_returns_error_but_upload_succeeds`.

## 2. Implementation

- [x] 2.1 Add `tesseract-plumbing = "0.11"` (optional) to `Cargo.toml`.
- [x] 2.2 Migration `0023_add_document_ocr.sql`.
- [x] 2.3 `src/ocr/mod.rs` — `OcrEngine` trait, `OcrResult`,
      `OcrError`.
- [x] 2.4 `src/ocr/tesseract.rs` — `TesseractEngine`
      implementation.
- [x] 2.5 `src/handlers/document_ocr.rs` — `show`,
      `run`, `apply` endpoints.
- [x] 2.6 `src/handlers/documents.rs` — enqueue OCR after
      upload unless `ocr=false`.
- [x] 2.7 `src/main.rs` — 2 new routes (via `lib.rs`).
- [x] 2.8 `src/templates/document_ocr.rs` Askama struct.
- [x] 2.9 `templates/documents/ocr.html` — display result,
      apply button.
- [x] 2.10 Update `templates/documents/list.html` — OCR
      status badge.

## 3. Validation

- [x] 3.1 `openspec validate receipt-ocr` passes.
- [x] 3.2 `cargo fmt --check` clean (new files).
- [x] 3.3 `cargo clippy --all-targets -- -D warnings` clean (no new errors).
- [x] 3.4 `cargo test` green (all 54 integration + 14 unit OCR tests).
- [ ] 3.5 Manual smoke: upload a real receipt JPEG → wait for
      OCR badge → click Apply → new reimbursement line has
      extracted amount / date / merchant.
- [x] 3.6 `openspec archive receipt-ocr`.
