-- Reversible: yes
-- DOWN for 0041_add_transaction_number.sql

ALTER TABLE transactions DROP COLUMN IF EXISTS number_year;
ALTER TABLE transactions DROP COLUMN IF EXISTS number;
DROP INDEX IF EXISTS idx_txn_number;

