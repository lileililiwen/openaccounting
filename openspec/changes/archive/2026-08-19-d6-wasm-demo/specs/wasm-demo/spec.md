# wasm-demo Specification (delta)

## ADDED Requirements

### Requirement: Browser-Only Execution

MUST run end-to-end in the browser with no network calls after initial load.

#### Scenario: Offline

- **WHEN** the user disconnects
- **THEN** the demo continues to work.

### Requirement: Seeded Data

MUST ship with a realistic 100-transaction ledger so the UI is non-trivial on first load.

#### Scenario: Seeded

- **WHEN** the page loads
- **THEN** the ledger shows 100+ transactions.

### Requirement: IndexedDB Persistence

MUST persist the user's edits in IndexedDB so refreshes keep state.

#### Scenario: Persistence

- **WHEN** the user adds a transaction and reloads
- **THEN** the new transaction is still there.

### Requirement: Bundle Size

MUST be ≤ 5 MB gzipped.

#### Scenario: Size

- **WHEN** the bundle
- **THEN** < 5 MB gzipped.

### Requirement: Read-Only Indication

MUST clearly indicate the demo URL is read-only-ish (writes persist locally but no server).

#### Scenario: Banner

- **WHEN** the page loads
- **THEN** a banner says 'Demo data; not saved to our servers'.
