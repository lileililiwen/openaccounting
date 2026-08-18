# ledger-export Specification (delta)

## ADDED Requirements

### Requirement: JSON Export

MUST export the full ledger as a single JSON document containing every account, transaction, posting, tag, document reference, and budget the user has access to; the format MUST be round-trippable (re-import produces identical rows).

#### Scenario: Export and round-trip

- **WHEN** the user exports ledger L and re-imports
- **THEN** the resulting rows are byte-equal to the original after canonicalization.

### Requirement: Beancount Export

MUST emit a Beancount-compatible text file with one `open`, one or more `txn` blocks, and `pad`/`balance` directives at the end.

#### Scenario: Beancount loads

- **WHEN** the user pipes the export into `bean-check`
- **THEN** the file parses with no errors.

### Requirement: Authorization

MUST require owner OR editor on the ledger.

#### Scenario: Viewer export

- **WHEN** a viewer tries to export
- **THEN** 403.

### Requirement: Streaming

MUST stream the export response so a 1-million-txn ledger does not OOM.

#### Scenario: Stream

- **WHEN** the user exports a large ledger
- **THEN** the server uses chunked transfer-encoding; memory stays bounded.

### Requirement: Consistency

MUST run the export in a single read-only transaction at REPEATABLE READ so the snapshot is internally consistent.

#### Scenario: Snapshot

- **WHEN** during the export a writer adds a transaction
- **THEN** the new transaction is NOT in the export.

### Requirement: Multicurrency

MUST preserve per-posting currency; the JSON includes both `amount` and `currency`; the Beancount output uses commodity annotations.

#### Scenario: Mixed currency

- **WHEN** the ledger has USD and EUR postings
- **THEN** the JSON contains both; the Beancount file declares both commodities.
