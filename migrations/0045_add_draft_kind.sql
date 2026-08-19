-- ============================================================================
-- 0045_add_draft_kind.sql — a8-draft-transactions
-- ============================================================================
-- Extends the `transactions.kind` CHECK constraint to include
-- 'draft' (work-in-progress, excluded from reports) and 'posted'
-- (synonym for the historical default 'standard'; reserved for
-- future use when the lifecycle gains a third terminal state).
-- ============================================================================

-- The constraint from 0005 was added inline without an explicit
-- name; Postgres auto-named it. We discover it dynamically so this
-- migration stays correct regardless of the underlying identifier.
DO $$
DECLARE
    constraint_name TEXT;
BEGIN
    SELECT con.conname
        INTO constraint_name
    FROM pg_constraint con
    JOIN pg_class rel ON rel.oid = con.conrelid
    WHERE rel.relname = 'transactions'
      AND con.contype = 'c'
      AND pg_get_constraintdef(con.oid) LIKE '%kind%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format(
            'ALTER TABLE transactions DROP CONSTRAINT %I',
            constraint_name);
    END IF;
END $$;

ALTER TABLE transactions ADD CONSTRAINT transactions_kind_check
    CHECK (kind IN ('standard', 'adjusting', 'closing', 'reversing',
                    'recurring', 'draft', 'posted', 'amortization'));
