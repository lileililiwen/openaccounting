-- Reversible: yes
-- DOWN for 0045_add_draft_kind.sql

ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_kind_check;
ALTER TABLE transactions ADD CONSTRAINT transactions_kind_check
    CHECK (kind IN ('standard', 'adjusting', 'closing', 'reversing'));

