DROP TABLE IF EXISTS payee_aliases;
DROP INDEX IF EXISTS uq_bsl_external;
ALTER TABLE bank_statement_lines DROP COLUMN IF EXISTS external_id;
