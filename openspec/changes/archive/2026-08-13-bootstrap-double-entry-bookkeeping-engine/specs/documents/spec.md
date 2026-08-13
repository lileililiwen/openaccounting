# documents Specification (delta)

## ADDED Requirements

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
