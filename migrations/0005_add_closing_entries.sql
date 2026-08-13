-- ============================================================================
-- 0005_add_closing_entries.sql — Closing entries, retained earnings, period lock
-- ============================================================================

-- Add kind column to transactions for distinguishing standard, adjusting, closing entries
ALTER TABLE transactions ADD COLUMN kind TEXT NOT NULL DEFAULT 'standard'
    CHECK (kind IN ('standard', 'adjusting', 'closing', 'reversing'));

-- Create closed_periods table to track which periods have been closed
CREATE TABLE IF NOT EXISTS closed_periods (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    period_year     INTEGER NOT NULL,
    closed_by       UUID NOT NULL REFERENCES users(id),
    closed_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, period_year)
);
CREATE INDEX IF NOT EXISTS idx_closed_periods_ledger ON closed_periods(ledger_id);

-- Add retained earnings account to existing ledgers that don't have one
INSERT INTO accounts (ledger_id, name, code, type, subtype, currency, description)
SELECT l.id, 'Retained Earnings', '3020', 'EQUITY', 'RETAINED_EARNINGS', l.base_currency, 'Accumulated profit/loss from prior years'
FROM ledgers l
WHERE NOT EXISTS (
    SELECT 1 FROM accounts a WHERE a.ledger_id = l.id AND a.subtype = 'RETAINED_EARNINGS'
);
