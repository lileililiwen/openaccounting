-- ============================================================================
-- 0042_add_investment_lots.sql — Lot-based cost-basis tracking
-- (`a6-investment-lots`).
-- ============================================================================
--
-- Lots are created by `buying` on an account of type `Investment`:
--   Dr Investment / Cr Cash
-- Disposals are recorded against the oldest lots first (FIFO).
--
-- `qty` and `unit_cost` are DECIMAL with high precision — fractional
-- shares are allowed.

CREATE TABLE IF NOT EXISTS investment_lots (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    acquired_at     DATE NOT NULL,
    qty             NUMERIC(20,8) NOT NULL CHECK (qty > 0),
    unit_cost       NUMERIC(20,8) NOT NULL CHECK (unit_cost >= 0),
    currency        CHAR(3) NOT NULL,
    source_txn_id   UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_investment_lots_account_acquired
    ON investment_lots (account_id, acquired_at, id);

CREATE TABLE IF NOT EXISTS investment_disposals (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lot_id          UUID NOT NULL REFERENCES investment_lots(id) ON DELETE CASCADE,
    qty             NUMERIC(20,8) NOT NULL CHECK (qty > 0),
    unit_proceeds   NUMERIC(20,8) NOT NULL CHECK (unit_proceeds >= 0),
    disposed_at     DATE NOT NULL,
    source_txn_id   UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    realized_gain   NUMERIC(20,8) NOT NULL,  -- proceeds - (qty * unit_cost_at_buy); can be negative
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_investment_disposals_lot
    ON investment_disposals (lot_id);
CREATE INDEX IF NOT EXISTS idx_investment_disposals_disposed_at
    ON investment_disposals (disposed_at DESC);