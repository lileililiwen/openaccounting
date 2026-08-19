-- ============================================================================
-- 0044_add_ledger_append_only.sql — d2-append-only-mode
-- ============================================================================
-- Adds a per-ledger append-only toggle. When TRUE:
--   * the PostingService refuses to mutate or delete transactions
--     (only reversal-via-PostingService is allowed);
--   * the accounts handler refuses to rename / reparent / archive
--     accounts in that ledger;
--   * the audit log records every toggle, including the prior value.
-- The toggle itself can only be changed by the ledger owner.
-- ============================================================================

ALTER TABLE ledgers
    ADD COLUMN IF NOT EXISTS append_only BOOLEAN NOT NULL DEFAULT FALSE;

-- Optional safety net: triggers that block direct UPDATEs / DELETEs on
-- the `transactions` table when the parent ledger is append-only.
-- Reversal entries are written via INSERT (not UPDATE), so they pass
-- the triggers naturally. We use two because Postgres expects BEFORE
-- UPDATE triggers to return NEW, and BEFORE DELETE triggers to return OLD.
CREATE OR REPLACE FUNCTION fn_block_edit_in_append_only()
RETURNS TRIGGER AS $$
DECLARE
    is_locked BOOLEAN;
BEGIN
    SELECT append_only INTO is_locked FROM ledgers WHERE id = OLD.ledger_id;
    IF is_locked THEN
        RAISE EXCEPTION 'ledger % is append-only; edits are not allowed', OLD.ledger_id
            USING ERRCODE = 'P0001';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_block_delete_in_append_only()
RETURNS TRIGGER AS $$
DECLARE
    is_locked BOOLEAN;
BEGIN
    SELECT append_only INTO is_locked FROM ledgers WHERE id = OLD.ledger_id;
    IF is_locked THEN
        RAISE EXCEPTION 'ledger % is append-only; deletes are not allowed', OLD.ledger_id
            USING ERRCODE = 'P0001';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION fn_block_account_edit_in_append_only()
RETURNS TRIGGER AS $$
DECLARE
    is_locked BOOLEAN;
BEGIN
    SELECT append_only INTO is_locked FROM ledgers WHERE id = OLD.ledger_id;
    IF is_locked THEN
        RAISE EXCEPTION 'ledger % is append-only; account edits are not allowed', OLD.ledger_id
            USING ERRCODE = 'P0001';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_block_txn_edit ON transactions;
DROP TRIGGER IF EXISTS trg_block_txn_delete ON transactions;
CREATE TRIGGER trg_block_txn_edit
    BEFORE UPDATE ON transactions
    FOR EACH ROW
    EXECUTE FUNCTION fn_block_edit_in_append_only();
CREATE TRIGGER trg_block_txn_delete
    BEFORE DELETE ON transactions
    FOR EACH ROW
    EXECUTE FUNCTION fn_block_delete_in_append_only();

DROP TRIGGER IF EXISTS trg_block_account_edit ON accounts;
CREATE TRIGGER trg_block_account_edit
    BEFORE UPDATE ON accounts
    FOR EACH ROW
    EXECUTE FUNCTION fn_block_account_edit_in_append_only();
