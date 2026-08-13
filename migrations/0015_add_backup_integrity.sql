-- ============================================================================
-- 0015_add_backup_integrity.sql — Backup tracking table
-- ============================================================================

CREATE TABLE IF NOT EXISTS backups (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename        TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    created_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('manual', 'scheduled')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_backups_created ON backups(created_at DESC);
