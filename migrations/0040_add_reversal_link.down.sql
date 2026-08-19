-- Reversible: yes
-- DOWN for 0040_add_reversal_link.sql

ALTER TABLE transactions DROP COLUMN IF EXISTS reverses_id;

