# documents Specification

## Purpose
TBD - created by archiving change bootstrap-double-entry-bookkeeping-engine. Update Purpose after archive.
## Requirements
### Requirement: Document Upload

A user MAY upload one or more files to a transaction via a
multipart form on the transaction detail page. Each uploaded file
SHALL be:

- Validated to be under 25 MiB (`MAX_BYTES = 25 * 1024 * 1024`).
- Restricted to the MIME allow-list:
  `image/png, image/jpeg, image/gif, image/webp, image/heic, image/heif, application/pdf, text/plain, text/csv`.
- Sanitized for filesystem storage: the original filename is passed
  through `sanitize-filename::sanitize` and prefixed with a fresh
  UUIDv4; the on-disk path is
  `{DOCUMENTS_DIR}/{transaction_id}/{uuid}-{sanitized}`.

The handler SHALL insert one `documents` row per accepted file, with
`uploaded_by = current_user.id`. The file is written to disk only
after the row insert succeeds (best-effort; failure cleans up the
file).

#### Scenario: Successful upload of a PDF receipt

- **WHEN** a user uploads a 1.2 MB PDF named `invoice-acme-2026-04.pdf`
  to transaction `txn-123`
- **THEN** a row is inserted into `documents` with
  `filename='invoice-acme-2026-04.pdf'`, `mime_type='application/pdf'`,
  `size_bytes=1258291`. The file exists on disk under
  `{DOCUMENTS_DIR}/txn-123/{uuid}-invoice-acme-2026-04.pdf`.

#### Scenario: Oversize file is rejected

- **WHEN** a user uploads a 30 MiB file
- **THEN** the handler returns HTTP 400 with body
  `File too large (max 26214400 bytes)`. No row is inserted.

#### Scenario: Disallowed MIME is rejected

- **WHEN** a user uploads `application/zip`
- **THEN** the handler returns HTTP 400 with body
  `Unsupported file type: application/zip`.

#### Scenario: Filename with path traversal is sanitized

- **WHEN** a user uploads a file with the name
  `../../etc/passwd`
- **THEN** the sanitized filename is `etcpasswd` (no `..`, no `/`)
  and the file is stored under the transaction's subdirectory.

### Requirement: Document Listing

The user MAY view all documents in a ledger at
`GET /ledgers/{id}/documents`. The page SHALL list every
`documents` row whose parent transaction is in the ledger, ordered
by `uploaded_at DESC`, showing:

- The original `filename` and `mime_type`.
- The parent transaction's `txn_date` and `description` as a link.
- The `size_bytes` (human-readable).
- A link to `GET /ledgers/{id}/documents/{doc_id}/download`.

On desktop the list is a table; on screens narrower than 768 px
(Tailwind `md`) it becomes a card list.

#### Scenario: Listing is ledger-scoped

- **WHEN** a user requests `GET /ledgers/<theirs>/documents`
- **THEN** the response only contains documents whose parent
  transaction's `ledger_id` matches. Documents from other ledgers
  are not visible.

### Requirement: Document Download

The endpoint
`GET /ledgers/{ledger_id}/documents/{doc_id}/download` SHALL return
the file's bytes with:

- HTTP 200 on success.
- `Content-Type` matching the stored `mime_type`.
- `Content-Disposition: inline; filename="<original>"`
- `Content-Length` matching the file size.
- HTTP 404 if the document does not exist or does not belong to the
  ledger.

#### Scenario: Owner downloads a document

- **WHEN** a user clicks "Open" on a document
- **THEN** the browser either previews the file inline (for PDF and
  image MIME types) or downloads it (others). No authentication
  challenge is presented again within the session.

#### Scenario: Non-owner requests a document

- **WHEN** user A requests
  `GET /ledgers/<B's ledger>/documents/<B's doc>/download`
- **THEN** the response is HTTP 404 (to avoid leaking existence).

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

`POST /ledgers/{id}/documents` MUST accept an optional query
parameter `ocr=false` (default `true`). When `false`, the
upload SHALL complete without enqueuing an OCR job; no
`document_ocr_results` row SHALL be created.

#### Scenario: Bulk PDF upload without OCR

- **WHEN** the user uploads a 50-page PDF bank statement with
  `ocr=false`
- **THEN** the upload completes immediately, no OCR job is
  enqueued, and `document_ocr_results` is empty for that
  document.

### Requirement: OCR Engine Pluggability

The OCR module SHALL expose a trait:

```rust
#[async_trait]
pub trait OcrEngine: Send + Sync {
    async fn extract(&self, bytes: &[u8], mime: &str)
        -> Result<OcrResult, OcrError>;
}
```

v1 MUST ship one implementation: `TesseractEngine`. Future
implementations (cloud providers) SHALL plug into the same trait.

#### Scenario: Default engine is Tesseract

- **WHEN** no `OCR_ENGINE` env var is set
- **THEN** the `TesseractEngine` is used.
- **WHEN** `OCR_ENGINE=textract` is set
- **THEN** startup fails with `OCR_ENGINE=textract requested
  but no implementation registered` (no Textract impl in v1).

