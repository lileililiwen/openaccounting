-- Reversible: yes
-- DOWN for 0009_add_recurring_transactions.sql

DROP TABLE IF EXISTS template_postings;
DROP TABLE IF EXISTS transaction_templates;
ALTER TABLE transactions DROP COLUMN IF EXISTS template_id;

