-- compliance-exports: per-period report notes (`period_notes`)
--
-- One notes record per (ledger, period_key) where period_key is a
-- free-form label the user picks (e.g. "2026-Q1", "2026-01").
-- Stores the body + author + timestamp for audit; rendered on P&L
-- and balance sheet print + PDF outputs.

CREATE TABLE IF NOT EXISTS period_notes (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id    uuid NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    period_key   text NOT NULL,
    body         text NOT NULL,
    created_by   uuid NOT NULL REFERENCES users(id),
    created_at   timestamp with time zone NOT NULL DEFAULT now(),
    updated_at   timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, period_key)
);

CREATE INDEX IF NOT EXISTS idx_period_notes_ledger ON period_notes(ledger_id);