-- ============================================================================
-- 0006_add_audit_trail.sql — Audit log for accountability and compliance
-- ============================================================================

CREATE TABLE IF NOT EXISTS audit_entries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID REFERENCES ledgers(id) ON DELETE SET NULL,
    actor_id        UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    action          TEXT NOT NULL,
    entity_type     TEXT NOT NULL,
    entity_id       UUID,
    old_value       JSONB,
    new_value       JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_audit_ledger_created ON audit_entries(ledger_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_actor_created ON audit_entries(actor_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_entity ON audit_entries(entity_type, entity_id);
