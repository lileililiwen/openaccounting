-- Migration 0032: Add notification_preferences table
-- (`u6-notification-preferences`).
--
-- One row per (user, channel, event) with a boolean opt-in.
-- The grid is sparse: rows are only inserted when the user
-- deviates from the per-channel default. Lookups
-- (`is_enabled`) apply the default when no row is present:
--
--   in_app       all events ON by default
--   email        weekly_summary ON; everything else OFF
--   push         all events OFF
--
-- Combined with the device_tokens table this gives a
-- per-user, per-channel, per-event opt-in.

CREATE TABLE IF NOT EXISTS notification_preferences (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel     TEXT NOT NULL
                CHECK (channel IN ('email', 'push', 'in_app')),
    event       TEXT NOT NULL
                CHECK (event IN (
                    'budget_overrun',
                    'reimbursement_submitted',
                    'large_transaction',
                    'weekly_summary'
                )),
    enabled     BOOLEAN NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (user_id, channel, event)
);

CREATE INDEX IF NOT EXISTS idx_notif_pref_user
    ON notification_preferences (user_id);