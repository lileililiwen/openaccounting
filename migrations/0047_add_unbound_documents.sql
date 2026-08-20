-- ============================================================================
-- 0047_add_unbound_documents.sql — document inbox (`a13-document-inbox`)
-- ============================================================================
--
-- Documents can now be uploaded without a transaction (unbound) and
-- bound to one later. `transaction_id` becomes nullable; a nullable
-- `ledger_id` anchors an unbound document to a ledger for listing,
-- authorization, and the inbox UI.
--
-- The reverse migration lives in 0047_add_unbound_documents.down.sql.

ALTER TABLE documents
    ALTER COLUMN transaction_id DROP NOT NULL;

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS ledger_id UUID REFERENCES ledgers(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_documents_ledger
    ON documents (ledger_id);
