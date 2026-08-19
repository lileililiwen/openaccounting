-- ============================================================================
-- 0043_add_inter_ledger_transfers.sql — Inter-ledger transfer link
-- (`a7-inter-ledger-transfers`).
-- ============================================================================
--
-- An inter-ledger transfer is two independent `transactions`
-- rows (one per ledger) linked by `inter_ledger_transfers.id`.
-- Each ledger's balance is preserved; the link table lets
-- consolidation reports eliminate duplicates.

CREATE TABLE IF NOT EXISTS inter_ledger_transfers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_ledger_id  UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    to_ledger_id    UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    from_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    to_account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    amount          NUMERIC(20,4) NOT NULL CHECK (amount > 0),
    currency        CHAR(3) NOT NULL,
    from_txn_id     UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    to_txn_id       UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    fee_amount      NUMERIC(20,4) NOT NULL DEFAULT 0 CHECK (fee_amount >= 0),
    fee_account_id  UUID REFERENCES accounts(id) ON DELETE SET NULL,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (from_ledger_id <> to_ledger_id),
    CHECK (from_txn_id <> to_txn_id)
);
CREATE INDEX IF NOT EXISTS idx_ilt_from_ledger
    ON inter_ledger_transfers (from_ledger_id);
CREATE INDEX IF NOT EXISTS idx_ilt_to_ledger
    ON inter_ledger_transfers (to_ledger_id);
