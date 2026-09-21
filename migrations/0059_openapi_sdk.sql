-- ============================================================================
-- 0059_openapi_sdk.sql — Automation rules + incoming event intake
-- ============================================================================
-- `openapi-sdk`: user-facing automation rules (event -> condition ->
-- action) executed on the existing jobs queue, plus a per-ledger
-- HMAC secret for signed incoming event intake.

CREATE TABLE IF NOT EXISTS automation_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    trigger         TEXT NOT NULL,
    conditions      JSONB NOT NULL DEFAULT '{}'::jsonb,
    action          TEXT NOT NULL
                    CHECK (action IN ('webhook_post', 'email_notify', 'categorize_transaction')),
    action_config   JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_enabled      BOOLEAN NOT NULL DEFAULT TRUE,
    created_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_automation_rules_ledger
    ON automation_rules(ledger_id, trigger) WHERE is_enabled = TRUE;

-- Per-ledger secret for signed incoming event intake
-- (`POST /api/events/{ledger_id}`, X-OA-Event-Signature HMAC).
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS incoming_events_secret TEXT;
