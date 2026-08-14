# Add Receipt OCR

## Why

Manually typing receipt details (amount, date, merchant) into
expense claim lines is the #1 friction point in any
reimbursement workflow. Expensify's killer feature is its
SmartScan OCR; Akaunting, Firefly III, and GnuCash all have
basic OCR integrations.

This change adds an OCR step to the existing `documents`
upload pipeline. After upload, a background job extracts
`amount`, `txn_date`, and `merchant` from the image (or the
embedded text in a PDF). The extracted values are surfaced in
the document detail page; the user clicks "Apply" to populate
a new (or existing) reimbursement line.

## What Changes

- New optional dependency: `tesseract-plumbing` for local OCR
  (no external service required for v1).
- New table `document_ocr_results`
  (`document_id`, `extracted_at`, `amount`, `txn_date`,
   `merchant`, `raw_text`, `engine`, `confidence`).
- New `POST /ledgers/{id}/documents/{doc_id}/ocr` endpoint that
  kicks off an OCR job; the response is `202 Accepted` and a
  background tokio task populates the row.
- New `GET /ledgers/{id}/documents/{doc_id}/ocr` endpoint that
  returns the result once ready.
- A "Apply to reimbursement line" button on the document page
  pre-fills a `POST /ledgers/{id}/reimbursements/{claim_id}/lines`
  form with the extracted values.
- New audit event: `document.ocr.run`, `document.ocr.apply`.

## Capabilities

### Modified Capabilities

- `documents` — adds OCR pipeline.

## Impact

- **New files:**
  - `migrations/0023_add_document_ocr.sql`
  - `src/ocr/mod.rs`
  - `src/ocr/tesseract.rs`
  - `src/handlers/document_ocr.rs`
  - `src/templates/document_ocr.rs`
  - `templates/documents/ocr.html`
  - `tests/integration/document_ocr.rs`
- **Modified files:**
  - `Cargo.toml` — `tesseract-plumbing = "0.13"`,
    `tokio = { features = ["rt-multi-thread"] }`.
  - `src/handlers/documents.rs` — enqueue OCR after upload.
  - `templates/documents/list.html` — show OCR status badge.
  - `src/main.rs` — 2 new routes.

## Non-Goals

- Cloud OCR providers (Textract, Azure FR, Google Document AI) —
  pluggable interface is designed but only Tesseract ships in
  v1.
- Line-item splitting (e.g. extracting multiple amounts from one
  receipt) — single amount only in v1.
- Multi-language OCR — Tesseract ships with eng+chi_sim trained
  data so simple bilingual receipts work; full locale switching
  is post-v1.
