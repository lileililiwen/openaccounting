-- Migration 0036: Add saved_searches table (`u2-saved-searches`).
--
-- Per-user (NOT per-ledger; the spec requires saved searches
-- to follow the user across ledgers they have access to). The
-- `query` field stores the raw query string the list endpoint
-- accepts (`from=...&to=...&q=...&account_id=...`). A boolean
-- `is_default` marks the row that the dashboard / list
-- should auto-apply; the partial UNIQUE INDEX keeps it
-- unique per user.

CREATE TABLE IF NOT EXISTS saved_searches (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    query       TEXT NOT NULL,
    color       TEXT
        CHECK (color IS NULL OR color IN (
            'slate', 'red', 'amber', 'emerald', 'sky', 'violet', 'pink'
        )),
    is_default  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)
);

-- Only one default per user.
CREATE UNIQUE INDEX IF NOT EXISTS uniq_saved_searches_default_per_user
    ON saved_searches (user_id)
    WHERE is_default = TRUE;

CREATE INDEX IF NOT EXISTS idx_saved_searches_user
    ON saved_searches (user_id);
