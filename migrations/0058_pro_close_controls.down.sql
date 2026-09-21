-- 0058_pro_close_controls.down.sql — reverse 0058 (reversible migration).

DROP INDEX IF EXISTS uq_invoices_number;
ALTER TABLE invoices DROP COLUMN IF EXISTS void_reason;
DROP TABLE IF EXISTS invoice_sequences;
ALTER TABLE ledgers DROP COLUMN IF EXISTS approval_threshold;
DROP TABLE IF EXISTS journal_approvals;
DROP TABLE IF EXISTS reopen_events;
DROP INDEX IF EXISTS idx_closed_periods_through;
ALTER TABLE closed_periods DROP COLUMN IF EXISTS closed_through;

-- Restore transactions.kind without 'pending'. Pending rows (if any) are
-- moved back to draft so the restore never violates the CHECK.
UPDATE transactions SET kind = 'draft' WHERE kind = 'pending';
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_kind_check;
ALTER TABLE transactions ADD CONSTRAINT transactions_kind_check
    CHECK (kind IN ('standard', 'adjusting', 'closing', 'reversing',
                    'recurring', 'draft', 'posted', 'amortization'));

-- Restore role enums. Non-legacy roles map to closest legacy equivalent.
UPDATE ledger_members SET role = 'editor' WHERE role IN ('accountant');
UPDATE ledger_members SET role = 'viewer' WHERE role IN ('auditor');
UPDATE ledger_invitations SET role = 'editor' WHERE role IN ('accountant');
UPDATE ledger_invitations SET role = 'viewer' WHERE role IN ('auditor');
ALTER TABLE ledger_members DROP CONSTRAINT IF EXISTS ledger_members_role_check;
ALTER TABLE ledger_members ADD CONSTRAINT ledger_members_role_check
    CHECK (role IN ('editor', 'viewer'));
ALTER TABLE ledger_invitations DROP CONSTRAINT IF EXISTS ledger_invitations_role_check;
ALTER TABLE ledger_invitations ADD CONSTRAINT ledger_invitations_role_check
    CHECK (role IN ('editor', 'viewer'));
