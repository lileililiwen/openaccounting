# documents Specification (delta)

## ADDED Requirements

### Requirement: Document OCR

When a document is uploaded, the system SHALL enqueue an OCR
job (unless the user opts out with `ocr=false` in the upload
form). The job runs in a background tokio task after the
upload handler returns `201 Created`; the user sees the
document immediately and a "OCR pending" badge.

When the job completes, the system SHALL populate
`document_ocr_results` with:

- `amount` — extracted decimal (nullable).
- `txn_date` — extracted date (nullable).
- `merchant` — extracted string (nullable).
- `raw_text` — full extracted text (always present).
- `engine` — `"tesseract"` (only engine in v1).
- `confidence` — float in `[0,1]` based on Tesseract's mean
  word confidence.

If extraction is unable to find an amount or date with
confidence ≥ 0.6, the corresponding field is stored as `NULL`.

#### Scenario: Clear receipt extracts cleanly

- **WHEN** the user uploads a JPEG of a clean supermarket
  receipt whose printed total is `¥123.45` and date is
  `2026-08-14`
- **AND WHEN** the OCR job runs
- **THEN** `amount = 123.45`, `txn_date = 2026-08-14`,
  `confidence ≥ 0.85`, `engine = "tesseract"`.

#### Scenario: Illegible receipt returns null fields

- **WHEN** the user uploads a heavily blurred photo
- **THEN** `amount = NULL`, `txn_date = NULL`, `raw_text`
  contains some characters, `confidence ≤ 0.6`.

### Requirement: OCR Apply

`POST /ledgers/{id}/documents/{doc_id}/ocr/apply` accepts:

- `claim_id` — required; the claim the line will be added to.
- `category` — required; one of the existing claim categories.
- `gl_account_id` — required; EXPENSE-type account.
- `tax_amount` — optional (default 0).

The handler MUST be authorized to add lines to the claim (the
caller must be the claim author or have `accountant` role).

On success the handler:

1. Adds a new `reimbursement_lines` row with the extracted
   `amount`, `txn_date`, `payee = merchant`, `description =
   raw_text` (truncated to 200 chars), and the form's
   `category`, `gl_account_id`, `tax_amount`.
2. Sets the line's `receipt_document_id` to `doc_id`.
3. Returns `303 See Other` to the claim show page.

#### Scenario: Apply creates a populated line

- **WHEN** the user clicks "Apply" on a document with
  `amount=123.45, txn_date=2026-08-14, merchant=Supermarket`
  and the form's `claim_id=<id of "Q3 client visits">`,
  `category=meals`, `gl_account_id=<id of "Office Supplies">`
- **THEN** a new line is added to that claim with the
  extracted values, and the response is `303 See Other`.

### Requirement: Skip OCR

`POST /ledgers/{id}/documents` accepts an optional form field
`ocr=false` (default `true`). When `false`, the upload
completes without enqueuing an OCR job; no
`document_ocr_results` row is created.

#### Scenario: Bulk PDF upload without OCR

- **WHEN** the user uploads a 50-page PDF bank statement with
  `ocr=false`
- **THEN** the upload completes immediately, no OCR job is
  enqueued, and `document_ocr_results` is empty for that
  document.

### Requirement: OCR Engine Pluggability

The OCR module exposes a trait:

```rust
#[async_trait]
pub trait OcrEngine: Send + Sync {
    async fn extract(&self, bytes: &[u8], mime: &str)
        -> Result<OcrResult, OcrError>;
}
```

v1 ships one implementation: `TesseractEngine`. Future
implementations (cloud providers) plug into the same trait.

#### Scenario: Default engine is Tesseract

- **WHEN** no `OCR_ENGINE` env var is set
- **THEN** the `TesseractEngine` is used.
- **WHEN** `OCR_ENGINE=textract` is set
- **THEN** startup fails with `OCR_ENGINE=textract requested
  but no implementation registered` (no Textract impl in v1).
