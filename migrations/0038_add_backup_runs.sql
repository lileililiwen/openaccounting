-- ============================================================================
-- 0038_add_backup_runs.sql — Track scheduled-backup runs (`o2-scheduled-backups`)
-- ============================================================================
--
-- A "run" is one execution attempt of the backup schedule. We log every
-- attempt (success or failure) so operators can audit whether backups
-- actually fired and what they produced.
--
-- The pre-existing `backups` table tracks files on disk; this table
-- tracks attempts and is independent of whether a file was written.

CREATE TABLE IF NOT EXISTS backup_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind            TEXT NOT NULL DEFAULT 'scheduled',  -- 'manual' | 'scheduled'
    scheduled_for   TIMESTAMPTZ NOT NULL,               -- the cron tick that triggered this run
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at     TIMESTAMPTZ,                        -- NULL while in-flight
    status          TEXT NOT NULL DEFAULT 'running'
                    CHECK (status IN ('running', 'success', 'failed')),
    filename        TEXT,                                -- basename of the tarball
    size_bytes      BIGINT,                              -- final tarball size on success
    error           TEXT                                 -- failure message
);
CREATE INDEX IF NOT EXISTS idx_backup_runs_started_at
    ON backup_runs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_backup_runs_status
    ON backup_runs(status);
