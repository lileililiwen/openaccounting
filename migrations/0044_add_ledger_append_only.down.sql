-- Reversible: yes
-- DOWN for 0044_add_ledger_append_only.sql

DROP TRIGGER IF EXISTS trg_block_txn_edit ON transactions;
DROP TRIGGER IF EXISTS trg_block_txn_delete ON transactions;
DROP TRIGGER IF EXISTS trg_block_account_edit ON accounts;
DROP FUNCTION IF EXISTS fn_block_edit_in_append_only();
DROP FUNCTION IF EXISTS fn_block_delete_in_append_only();
DROP FUNCTION IF EXISTS fn_block_account_edit_in_append_only();
ALTER TABLE ledgers DROP COLUMN IF EXISTS append_only;

