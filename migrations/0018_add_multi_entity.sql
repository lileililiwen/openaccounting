-- ============================================================================
-- 0018_add_multi_entity.sql — Multi-entity support with consolidation
-- ============================================================================

CREATE TABLE IF NOT EXISTS entities (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    legal_name      TEXT,
    tax_id          TEXT,
    owner_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_entities_owner ON entities(owner_id);

ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS entity_id UUID REFERENCES entities(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_ledgers_entity ON ledgers(entity_id);
