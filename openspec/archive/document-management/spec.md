# document-management Specification

## Purpose
Organize and manage financial documents (invoices, receipts, contracts) with categorization, search, and multi-file upload support.

## Requirements

### Requirement: Document Categories

The system MUST support categorizing documents with predefined
categories:

- Invoice
- Receipt
- Contract
- Bank Statement
- Tax Document
- Other

The category is stored as a `category` field on the `documents`
table. Users MUST be able to filter documents by category.

#### Scenario: Document is categorized

- **WHEN** a user uploads a receipt and selects category="Receipt"
- **THEN** the document is stored with `category='Receipt'` and
  appears when filtering by "Receipt".

### Requirement: Multi-File Upload

The document upload interface MUST support selecting and
uploading multiple files in a single action. Each file MUST
be stored as a separate document record.

#### Scenario: Multiple files are uploaded

- **WHEN** a user selects 3 files and clicks upload
- **THEN** all 3 files are uploaded and 3 document records are
  created.

### Requirement: Document Search

The system MUST support searching documents by:

- Filename (partial match, case-insensitive)
- Category (exact match)
- Date range (upload date)
- Tag (exact match)

The search MUST be available on the documents page.

#### Scenario: User searches for receipts

- **WHEN** a user filters documents by category="Receipt" and
  date range Jan-Feb 2026
- **THEN** only documents with category="Receipt" uploaded in
  that date range are shown.

### Requirement: Document Tags

Documents MUST support optional tags (comma-separated on upload
or edit). Tags enable cross-cutting classification (e.g.,
"Q1-2026", "tax-deductible", "office-expenses").

The system MUST maintain a `document_tags` junction table:

- `document_id` UUID
- `tag` TEXT

Tags MUST be searchable and filterable.

#### Scenario: Document is tagged

- **WHEN** a user uploads a document with tags "office, rent"
- **THEN** the document can be found by searching for "office"
  or "rent".

### Requirement: Document Deletion

Users MUST be able to delete documents they own. Deletion MUST
remove both the database record and the file from storage.

#### Scenario: User deletes a document

- **WHEN** a user clicks delete on a document they own
- **THEN** the document record and file are removed, and the
  documents page refreshes.

### Requirement: Document Upload to Storage

Uploaded files MUST be stored on disk (or object storage) with
a unique filename. The `documents` table stores only metadata
(name, size, content type, path); the actual file content is
NOT stored in the database.

#### Scenario: File is stored on disk

- **WHEN** a user uploads a 2.5MB PDF invoice
- **THEN** the file is saved to disk with a unique name (e.g.,
  UUID-based) and the `documents` table stores the path.
