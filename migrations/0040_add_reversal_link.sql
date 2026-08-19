-- ============================================================================
-- 0040_add_reversal_link.sql — Reversal link on transactions
-- (`a2-transaction-edit-void`).
-- ============================================================================
--
-- A reversal is a regular transaction whose postings are the
-- negation of the original's. We keep the original intact (full
-- audit) and link the reversal back to it via `reverses_id`.
-- Two reversals of the same original cancel out at the net
-- level — that is the canonical accounting pattern and is
-- allowed.

ALTER TABLE transactions ADD COLUMN reverses_id UUID
    REFERENCES transactions(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_txn_reverses_id ON transactions(reverses_id);
