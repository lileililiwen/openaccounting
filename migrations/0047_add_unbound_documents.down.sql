-- ============================================================================
-- 0047_add_unbound_documents.down.sql — reverse of 0047
-- ============================================================================

DROP INDEX IF EXISTS idx_documents_ledger;

ALTER TABLE documents
    DROP COLUMN IF EXISTS ledger_id;

-- Reversible: no — re-adding NOT NULL would fail on unbound rows.
