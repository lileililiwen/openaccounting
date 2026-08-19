-- Reversible: yes
-- DOWN for 0037_add_audit_chain.sql

ALTER TABLE audit_entries DROP COLUMN IF EXISTS prev_hash;
ALTER TABLE audit_entries DROP COLUMN IF EXISTS row_hash;

