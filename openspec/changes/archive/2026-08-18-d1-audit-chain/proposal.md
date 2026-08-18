# Cryptographic Audit Chain

## Why

The `audit` table (`migrations/0006_add_audit_trail.sql`) records
who did what, but rows are mutable at the DB level (any superuser can
UPDATE or DELETE them). No peer offers tamper detection. Adding a
hash-chain turns the audit log into a Merkle-style append-only ledger.

## What Changes

- Each `audit` row gains `prev_hash BYTEA` and `hash BYTEA`.
- `hash = SHA256(prev_hash || row_bytes)`.
- On insert, the application reads the latest row, copies its hash,
  computes the new hash, and inserts.
- Verification endpoint walks the chain; reports break points.
- Optional companion: publish a daily Merkle root to a write-only log
  file (off-chain tamper evidence).

## Capabilities

### New Capabilities

- `audit-chain`: Hash-linked audit log.

## Impact

**New files:**
- `migrations/0045_add_audit_chain.sql`.
- `src/audit/chain.rs`.
- `src/handlers/admin_audit_verify.rs`.
- `tests/integration/audit_chain.rs`.
