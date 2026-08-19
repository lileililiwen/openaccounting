-- Reversible: yes
-- DOWN for 0018_add_multi_entity.sql

DROP TABLE IF EXISTS inter_entity_transactions;
DROP TABLE IF EXISTS entities;
ALTER TABLE ledgers DROP COLUMN IF EXISTS entity_id;

