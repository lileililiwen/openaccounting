# document-authorization Specification (delta)

## ADDED Requirements

### Requirement: Download Authorization

MUST verify that the requesting user has access (owner, editor, viewer, or admin) to the ledger that owns the document's transaction; otherwise respond 404.

#### Scenario: Owner download

- **WHEN** the ledger owner requests a document id of their own ledger
- **THEN** the document bytes are returned.

#### Scenario: Editor download

- **WHEN** an editor requests the same document
- **THEN** the document bytes are returned.

#### Scenario: Viewer download

- **WHEN** a viewer requests the same document
- **THEN** the document bytes are returned.

#### Scenario: Cross-ledger user

- **WHEN** a user with no relation to the ledger requests the document
- **THEN** the response is 404.

#### Scenario: Cross-ledger same user, different ledger

- **WHEN** the same user is owner of ledger A and tries to read a document in ledger B
- **THEN** the response is 404.

#### Scenario: Anonymous

- **WHEN** no session cookie is sent
- **THEN** redirect to /login (handled by existing auth layer).

### Requirement: Mutating Authorization

MUST apply the same authorization to `POST /documents/{id}/delete` and any OCR run.

#### Scenario: Cross-ledger delete

- **WHEN** user A from ledger B tries to delete a doc in ledger C
- **THEN** 404.
