# ## Context

Tamper-evident logs are well-known (certificate transparency,
sigstore). None of the bookkeeping peers offer this.

## Goals / Non-Goals

**Goals:**
- Detect retroactive edits of the audit log.

**Non-Goals:**
- Cryptographic proof of an individual user's identity (we already have
  the user_id column).

## Decisions

- SHA-256 is fine; not signing with a key — that's a future change.
- The DB constraint is the strong part; the daily anchor is just a
  backup.

## Risks / Trade-offs

- Backfilling hashes is O(N) on migration; tolerable for v1 scale.
- DB-level UPDATE/DELETE revocation requires a migration that revokes
  privileges from the application user. We do that in the same
  migration.
