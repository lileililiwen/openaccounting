-- Reversible: yes
-- DOWN for 0020_add_ledger_basis.sql

ALTER TABLE ledgers DROP CONSTRAINT IF EXISTS chk_ledger_basis;
ALTER TABLE ledgers DROP COLUMN IF EXISTS basis;

