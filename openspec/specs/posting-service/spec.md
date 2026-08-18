# posting-service Specification

## Purpose
TBD - created by archiving change a3-posting-service. Update Purpose after archive.
## Requirements
### Requirement: Single Create Entry Point

MUST route every transaction creation through `PostingService::create(NewTransaction)`; no other module MAY write to `transactions` or `postings`.

#### Scenario: Direct DB write blocked

- **WHEN** a handler bypasses PostingService
- **THEN** a clippy lint / code-review gate rejects the change.

### Requirement: Invariant Enforcement

MUST reject any input whose postings do not balance to zero; the error MUST be raised before any DB row is inserted.

#### Scenario: Unbalanced rejected

- **WHEN** PostingService is called with debits=100, credits=99
- **THEN** no rows inserted; error returned.

### Requirement: Period-Close Check

MUST reject writes to closed fiscal years.

#### Scenario: Closed period

- **WHEN** PostingService is called with a date in a closed year
- **THEN** rejected; no rows inserted.

### Requirement: Atomicity

MUST insert transaction + postings in a single Postgres transaction; either all rows commit or none do.

#### Scenario: Atomic

- **WHEN** a simulated failure after postings insert
- **THEN** the transaction row is rolled back.

### Requirement: Audit Emission

MUST emit one audit-log row per successful write; the audit MUST include the before/after JSON of the transaction.

#### Scenario: Audit

- **WHEN** a successful write
- **THEN** one audit row exists with action=create and the full payload.

### Requirement: Concurrency Lock

MUST `SELECT … FOR UPDATE` on the parent ledger row before inserting, to prevent concurrent writers from interleaving.

#### Scenario: Concurrent writers

- **WHEN** two writes hit the same ledger simultaneously
- **THEN** one waits for the other; both succeed in order.

