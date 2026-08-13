-- ============================================================================
-- 0016_add_fixed_assets.sql — Fixed asset tracking with depreciation
-- ============================================================================

CREATE TABLE IF NOT EXISTS fixed_assets (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id               UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    description             TEXT,
    account_id              UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    purchase_date           DATE NOT NULL,
    purchase_cost           NUMERIC(20,4) NOT NULL,
    salvage_value           NUMERIC(20,4) NOT NULL DEFAULT 0,
    useful_life_years       INTEGER NOT NULL,
    depreciation_method     TEXT NOT NULL CHECK (depreciation_method IN ('straight_line', 'declining_balance')),
    accumulated_depreciation NUMERIC(20,4) NOT NULL DEFAULT 0,
    status                  TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disposed', 'fully_depreciated')),
    disposed_date           DATE,
    disposed_amount         NUMERIC(20,4),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_fixed_assets_ledger ON fixed_assets(ledger_id);
