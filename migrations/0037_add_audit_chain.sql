-- ============================================================================
-- 0037_add_audit_chain.sql — Hash-chain on audit_entries
-- ----------------------------------------------------------------------------
-- Adds `prev_hash` + `hash` columns so the audit log becomes an
-- append-only Merkle-style chain (`d1-audit-chain`).
--
--   hash = SHA256(prev_hash || canonical_row_bytes)
--
-- The backfill of pre-existing rows is done by the application at
-- startup (`crate::audit::chain::ensure_backfilled`) because the
-- canonical serialization lives in Rust and would otherwise drift
-- from a PL/pgSQL copy. New rows are hashed by the app on insert.
-- ============================================================================

ALTER TABLE audit_entries
    ADD COLUMN IF NOT EXISTS prev_hash BYTEA;

ALTER TABLE audit_entries
    ADD COLUMN IF NOT EXISTS hash BYTEA;

-- Hash-chain lookups walk the chain in insertion order.
CREATE INDEX IF NOT EXISTS idx_audit_chain_order
    ON audit_entries (created_at ASC, id ASC);
