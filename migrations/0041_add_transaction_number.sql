-- ============================================================================
-- 0041_add_transaction_number.sql — Per-ledger-per-year transaction number
-- (`a5-transaction-numbering`).
-- ============================================================================
--
-- `transactions.number TEXT` is the human-citable reference. Format is
-- free-form (so users can supply their own invoice numbers, check
-- numbers, etc.) but uniqueness is enforced per (ledger_id, year).
--
-- We derive `year` from `txn_date` at read time via a generated column,
-- so the index is purely on (ledger_id, year, number).

ALTER TABLE transactions ADD COLUMN number TEXT;
ALTER TABLE transactions ADD COLUMN number_year INTEGER
    GENERATED ALWAYS AS (EXTRACT(YEAR FROM txn_date)::INTEGER) STORED;

CREATE UNIQUE INDEX IF NOT EXISTS uq_transactions_number
    ON transactions (ledger_id, number_year, number)
    WHERE number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_transactions_number
    ON transactions (ledger_id, number);
