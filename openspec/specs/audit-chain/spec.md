# audit-chain Specification

## Purpose
TBD - created by archiving change d1-audit-chain. Update Purpose after archive.
## Requirements
### Requirement: Hash Computation

MUST compute `hash = SHA256(prev_hash || canonical_row_bytes)` for every audit insert; the canonical form MUST be a deterministic JSON serialization.

#### Scenario: Insert

- **WHEN** an audit row is written
- **THEN** the row contains `prev_hash` from the prior row and `hash` from SHA256.

### Requirement: Chain Integrity

MUST expose `/admin/audit/verify` that walks the chain in order and reports the first broken link (or success).

#### Scenario: Clean chain

- **WHEN** no one has tampered
- **THEN** the endpoint returns 200 with `{ ok: true, length: N }`.

#### Scenario: Tampered

- **WHEN** a row's `hash` is mutated
- **THEN** the endpoint returns 200 with `{ ok: false, broken_at: <id> }`.

### Requirement: Append-Only Enforcement

MUST revoke UPDATE/DELETE on the `audit` table for the application role; a separate maintenance role retains them.

#### Scenario: App role update

- **WHEN** the app role issues UPDATE
- **THEN** permission denied.

### Requirement: Backward Compatibility

MUST back-fill the `prev_hash`/`hash` columns for existing rows on migration by chaining them in order.

#### Scenario: Backfill

- **WHEN** the migration runs
- **THEN** all existing rows have hashes.

### Requirement: Daily Anchor

MUST publish the latest hash to `data/audit-anchor.log` once per day; the file is append-only (chattr +a on Linux).

#### Scenario: Daily

- **WHEN** the worker runs
- **THEN** the anchor file has one new line per day.

