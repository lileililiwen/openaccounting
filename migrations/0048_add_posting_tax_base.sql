-- ============================================================================
-- 0048_add_posting_tax_base.sql — Track the taxable (net) base per tax link
-- ============================================================================
-- The tax report needs net / tax / gross per rate. `posting_taxes` already
-- stores `tax_amount`; this adds the base amount the tax was computed on
-- (line amount before tax). NULL is treated as 0 by the report for rows
-- created before this migration.
-- ============================================================================

ALTER TABLE posting_taxes ADD COLUMN IF NOT EXISTS base_amount NUMERIC(20,4);
