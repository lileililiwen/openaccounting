-- ============================================================================
-- 0051_automation_platform.sql — Job queue, outgoing webhooks, reminders
-- ============================================================================
-- `automation-platform`: scheduler-worker, notification-channels,
-- outgoing-webhooks.
--
--   1. `jobs` — DB-backed queue claimed with FOR UPDATE SKIP LOCKED.
--   2. `webhook_subscriptions` / `webhook_deliveries` — HMAC-signed
--      outbound events; one delivery row per attempt (spec shape).
--   3. `invoice_reminders` — idempotency for overdue reminders at
--      day 1/7/14 offsets.
--   4. Unique index making recurring-template occurrences idempotent
--      by (template_id, due_date).
--   5. notification_preferences gains the 'invoice_overdue' event.
-- ============================================================================

CREATE TABLE IF NOT EXISTS jobs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind         TEXT NOT NULL,
    payload      JSONB NOT NULL DEFAULT '{}',
    run_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts     INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    status       TEXT NOT NULL DEFAULT 'pending'
                 CHECK (status IN ('pending', 'running', 'done', 'dead')),
    last_error   TEXT,
    created_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_jobs_due ON jobs (status, run_at);

CREATE TABLE IF NOT EXISTS webhook_subscriptions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    -- https always; plain-http allowed ONLY for loopback (tests/dev).
    target_url  TEXT NOT NULL CHECK (
                    target_url LIKE 'https://%'
                    OR target_url LIKE 'http://127.0.0.1:%'
                ),
    secret      TEXT NOT NULL,
    events      TEXT[] NOT NULL,
    is_enabled  BOOLEAN NOT NULL DEFAULT TRUE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_webhook_subs_ledger ON webhook_subscriptions(ledger_id);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES webhook_subscriptions(id) ON DELETE CASCADE,
    event_id        UUID NOT NULL,
    event_type      TEXT NOT NULL,
    payload         JSONB NOT NULL,
    attempt         INT NOT NULL DEFAULT 0,
    ok              BOOLEAN,
    status_code     INT,
    duration_ms     INT,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subscription_id, event_id, attempt)
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliv_sub ON webhook_deliveries(subscription_id, event_id);

CREATE TABLE IF NOT EXISTS invoice_reminders (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    offset_day INT NOT NULL,
    channel    TEXT NOT NULL,
    skipped    BOOLEAN NOT NULL DEFAULT FALSE,
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (invoice_id, offset_day, channel)
);

-- Recurring occurrences post at most once per (template, due date).
CREATE UNIQUE INDEX IF NOT EXISTS uq_txn_template_due
    ON transactions (template_id, txn_date)
    WHERE template_id IS NOT NULL;

ALTER TABLE notification_preferences DROP CONSTRAINT IF EXISTS notification_preferences_event_check;
ALTER TABLE notification_preferences ADD CONSTRAINT notification_preferences_event_check
    CHECK (event IN (
        'budget_overrun',
        'reimbursement_submitted',
        'large_transaction',
        'weekly_summary',
        'invoice_overdue'
    ));

-- Generic in-app notification feed (`notification-channels`).
CREATE TABLE IF NOT EXISTS in_app_notifications (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event      TEXT NOT NULL,
    message    TEXT NOT NULL,
    read_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_inapp_user ON in_app_notifications(user_id, created_at DESC);

-- Per-user generic HTTP delivery target (ntfy/Gotify-compatible)
-- (`notification-channels`).
CREATE TABLE IF NOT EXISTS notification_http_targets (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    target_url TEXT NOT NULL CHECK (target_url LIKE 'https://%'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
