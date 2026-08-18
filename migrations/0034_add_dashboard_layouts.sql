-- Migration 0034: Add dashboard_layouts table (`u5-dashboard-widgets`).
--
-- One row per (user_id, ledger_id). `widgets` is a TEXT[] in the
-- order the user wants them rendered. Empty / NULL falls back
-- to the built-in default ordering.
--
-- The CHECK constraint pins the widget vocabulary to the eight
-- IDs shipped in day-1; adding a new widget is a code change
-- that migrates existing rows.

CREATE TABLE IF NOT EXISTS dashboard_layouts (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    widgets     TEXT[] NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, ledger_id),
    CONSTRAINT widgets_vocabulary CHECK (
        widgets <@ ARRAY['kpis', 'cash_runway', 'charts', 'top_expenses', 'recent_txns', 'budget_burn', 'account_balances']
    )
);

CREATE INDEX IF NOT EXISTS idx_dashboard_layouts_user
    ON dashboard_layouts (user_id);