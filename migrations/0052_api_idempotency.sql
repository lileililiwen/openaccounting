-- ============================================================================
-- 0052_api_idempotency.sql — Durable idempotency keys for /api/v1
-- ============================================================================
-- `api-v2-coverage`: replaces the per-process HashMap that lost keys on
-- restart. One row per (key, token); replays within 24 h return the
-- stored response without re-executing.
-- ============================================================================

CREATE TABLE IF NOT EXISTS api_idempotency (
    key                 TEXT NOT NULL,
    token_id            UUID NOT NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
    request_fingerprint TEXT NOT NULL,
    response_status     INT NOT NULL,
    response_body       TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (key, token_id)
);

CREATE INDEX IF NOT EXISTS idx_api_idem_created ON api_idempotency(created_at);
