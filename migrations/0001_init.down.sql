-- Reversible: yes
-- DOWN for 0001_init.sql

DROP TRIGGER IF EXISTS trg_transactions_updated ON transactions;
DROP TRIGGER IF EXISTS trg_posting_balance ON postings;
DROP FUNCTION IF EXISTS set_updated_at();
DROP FUNCTION IF EXISTS check_posting_balance();
DROP TABLE IF EXISTS postings;
DROP TABLE IF EXISTS transactions;
DROP TABLE IF EXISTS accounts;
DROP TABLE IF EXISTS ledgers;
DROP INDEX IF EXISTS idx_users_email_lower;
DROP TABLE IF EXISTS users;

