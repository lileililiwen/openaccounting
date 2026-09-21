-- 0056_statement_reconciliation.down.sql — reverse 0056 (reversible migration).

DROP INDEX IF EXISTS idx_bsl_account_date;
DROP TABLE IF EXISTS rec_lines;
DROP TABLE IF EXISTS rec_sessions;
