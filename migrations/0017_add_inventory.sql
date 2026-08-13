-- ============================================================================
-- 0017_add_inventory.sql — Inventory tracking with FIFO cost flow
-- ============================================================================

CREATE TABLE IF NOT EXISTS inventory_items (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id           UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    sku                 TEXT,
    description         TEXT,
    asset_account_id    UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    cogs_account_id     UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    income_account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    quantity_on_hand    INTEGER NOT NULL DEFAULT 0,
    unit_cost           NUMERIC(20,4) NOT NULL DEFAULT 0,
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_inventory_ledger ON inventory_items(ledger_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_inventory_sku ON inventory_items(ledger_id, sku) WHERE sku IS NOT NULL;

CREATE TABLE IF NOT EXISTS inventory_layers (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id             UUID NOT NULL REFERENCES inventory_items(id) ON DELETE CASCADE,
    quantity            INTEGER NOT NULL,
    unit_cost           NUMERIC(20,4) NOT NULL,
    remaining           INTEGER NOT NULL,
    purchase_date       DATE NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_inv_layers_item ON inventory_layers(item_id, purchase_date);
