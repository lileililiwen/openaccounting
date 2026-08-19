-- Reversible: yes
-- DOWN for 0004_add_account_subtypes.sql

ALTER TABLE accounts DROP CONSTRAINT IF EXISTS chk_account_subtype;
ALTER TABLE accounts DROP COLUMN IF EXISTS subtype;
ALTER TABLE accounts DROP COLUMN IF EXISTS description;
ALTER TABLE accounts DROP COLUMN IF EXISTS updated_at;

