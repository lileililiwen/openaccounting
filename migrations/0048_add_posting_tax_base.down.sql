-- DOWN for 0048_add_posting_tax_base.sql

ALTER TABLE posting_taxes DROP COLUMN IF EXISTS base_amount;
