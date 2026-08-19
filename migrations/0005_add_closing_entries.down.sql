-- Reversible: yes
-- DOWN for 0005_add_closing_entries.sql

ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_kind_check;
ALTER TABLE transactions DROP COLUMN IF EXISTS kind;
DROP TABLE IF EXISTS closed_periods;
DELETE FROM accounts WHERE subtype = 'RETAINED_EARNINGS';
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS chk_account_subtype;
ALTER TABLE accounts ADD CONSTRAINT chk_account_subtype CHECK (
    (type = 'ASSET'     AND subtype IN ('CURRENT','FIXED','INTANGIBLE','OTHER')) OR
    (type = 'LIABILITY' AND subtype IN ('CURRENT','LONG_TERM')) OR
    (type = 'EQUITY'    AND subtype IN ('EQUITY')) OR
    (type = 'INCOME'    AND subtype IN ('OPERATING','NON_OPERATING')) OR
    (type = 'EXPENSE'   AND subtype IN ('OPERATING','NON_OPERATING','COGS'))
);

