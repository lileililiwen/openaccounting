-- ============================================================================
-- 0020_add_ledger_basis.sql — Cash-basis reporting toggle
-- ============================================================================
-- Each ledger now has a `basis` of either 'accrual' (default) or
-- 'cash'. The income-statement and cash-flow report handlers
-- accept `?basis=...`; absent the parameter, the ledger's stored
-- basis is used.
--
-- Existing rows are backfilled to 'accrual' by the DEFAULT clause.
-- ============================================================================

ALTER TABLE ledgers
    ADD COLUMN basis TEXT NOT NULL DEFAULT 'accrual'
    CHECK (basis IN ('accrual', 'cash'));
