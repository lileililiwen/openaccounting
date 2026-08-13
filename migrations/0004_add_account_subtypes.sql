-- ============================================================================
-- 0004_add_account_subtypes.sql — Account subtypes for classified reporting
-- ============================================================================

-- Add subtype column with a CHECK constraint per type
ALTER TABLE accounts ADD COLUMN subtype TEXT NOT NULL DEFAULT 'CURRENT_ASSET';

-- Set appropriate defaults for existing accounts based on their type and name
UPDATE accounts SET subtype = 'CURRENT_ASSET' WHERE type = 'ASSET' AND name IN ('Cash on Hand', 'Bank Account', 'Accounts Receivable');
UPDATE accounts SET subtype = 'CURRENT_LIABILITY' WHERE type = 'LIABILITY' AND name IN ('Accounts Payable', 'Credit Card');
UPDATE accounts SET subtype = 'EQUITY' WHERE type = 'EQUITY';
UPDATE accounts SET subtype = 'OPERATING_INCOME' WHERE type = 'INCOME' AND name IN ('Sales Revenue', 'Other Income');
UPDATE accounts SET subtype = 'OPERATING_EXPENSE' WHERE type = 'EXPENSE' AND name IN ('Office Supplies', 'Travel & Meals', 'Software & SaaS', 'Marketing', 'Professional Services', 'Rent', 'Utilities', 'Other Expense');

-- Add CHECK constraint for valid subtype/type combinations
ALTER TABLE accounts ADD CONSTRAINT chk_account_subtype CHECK (
    (type = 'ASSET'     AND subtype IN ('CURRENT_ASSET', 'FIXED_ASSET', 'INTANGIBLE_ASSET', 'OTHER_ASSET')) OR
    (type = 'LIABILITY' AND subtype IN ('CURRENT_LIABILITY', 'LONG_TERM_LIABILITY')) OR
    (type = 'EQUITY'    AND subtype IN ('EQUITY', 'RETAINED_EARNINGS', 'DRAWING')) OR
    (type = 'INCOME'    AND subtype IN ('OPERATING_INCOME', 'NON_OPERATING_INCOME')) OR
    (type = 'EXPENSE'   AND subtype IN ('COST_OF_GOODS_SOLD', 'OPERATING_EXPENSE', 'NON_OPERATING_EXPENSE', 'TAX_EXPENSE'))
);

-- Index for querying by subtype
CREATE INDEX IF NOT EXISTS idx_accounts_subtype ON accounts(ledger_id, subtype);
