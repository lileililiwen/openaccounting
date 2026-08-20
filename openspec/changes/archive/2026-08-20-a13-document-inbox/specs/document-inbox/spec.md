# document-inbox Specification (delta)

## ADDED Requirements

### Requirement: Unbound Document Upload

MUST allow a ledger writer to upload one or more documents to the ledger without a transaction, via `POST /ledgers/{id}/documents`; each upload SHALL be stored with a NULL `transaction_id` and the ledger's id as its authorization anchor; uploads MUST reuse the existing MIME validation and storage path.

#### Scenario: Upload without a transaction

- **WHEN** a ledger writer uploads a file from the documents page
- **THEN** an unbound document row is created (`transaction_id` NULL) and appears in the ledger's documents inbox.

#### Scenario: Non-writer blocked

- **WHEN** a viewer or non-member attempts the ledger-level upload
- **THEN** 403.

### Requirement: Document Inbox

MUST list unbound documents in the ledger Documents view, visually distinct from bound documents, each with a "bind to transaction" action available to writers.

#### Scenario: Unbound listed

- **WHEN** an unbound document exists for the ledger
- **THEN** it appears in the documents list with a bind action.

#### Scenario: Bound documents still listed

- **WHEN** documents are attached to transactions
- **THEN** they remain listed with their transaction, as today.

### Requirement: Bind Document to Transaction

MUST allow binding an unbound document to a transaction: the writer searches transactions by date, amount, or description, or creates a new transaction, and confirms the bind; binding SHALL set the document's `transaction_id` and SHALL be recorded in the audit log.

#### Scenario: Bind by search

- **WHEN** a writer binds a receipt to a matching transaction found by search
- **THEN** the document's `transaction_id` is set and the audit log records the bind.

#### Scenario: Bind by creating a transaction

- **WHEN** no matching transaction exists
- **THEN** the writer can create a transaction and bind the document to it in the same flow.

#### Scenario: Bind requires writer

- **WHEN** a viewer attempts to bind
- **THEN** 403.

### Requirement: Unbound Document Authorization

MUST enforce access for unbound documents: only the ledger's writers SHALL download an unbound document's file; after binding, the existing member rule applies; non-members SHALL receive 404.

#### Scenario: Writer downloads unbound

- **WHEN** a ledger writer requests an unbound document's file
- **THEN** the file is served.

#### Scenario: Non-member blocked

- **WHEN** a non-member requests an unbound document's file
- **THEN** 404.
