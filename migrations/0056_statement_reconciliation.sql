-- ============================================================================
-- 0056_statement_reconciliation.sql — Statement reconciliation sessions
-- ============================================================================
-- Statement-level workflow (`statement-reconciliation`): a session ties
-- one account + one statement period to an external closing balance.
-- Cleared state lives in `rec_lines` per statement line so re-running a
-- period never rewrites history; a closed session locks its lines.

CREATE TABLE IF NOT EXISTS rec_sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id           UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    account_id          UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    stmt_close_date     DATE NOT NULL,
    stmt_close_balance  NUMERIC(20,4) NOT NULL,
    opening_balance     NUMERIC(20,4) NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_by          UUID REFERENCES users(id) ON DELETE SET NULL,
    closed_by           UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at           TIMESTAMPTZ,
    UNIQUE (account_id, stmt_close_date)
);
CREATE INDEX IF NOT EXISTS idx_rec_sessions_account ON rec_sessions(account_id, stmt_close_date);
CREATE INDEX IF NOT EXISTS idx_rec_sessions_ledger ON rec_sessions(ledger_id);

CREATE TABLE IF NOT EXISTS rec_lines (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      UUID NOT NULL REFERENCES rec_sessions(id) ON DELETE CASCADE,
    bank_line_id    UUID NOT NULL REFERENCES bank_statement_lines(id) ON DELETE CASCADE,
    cleared         BOOLEAN NOT NULL DEFAULT FALSE,
    cleared_at      TIMESTAMPTZ,
    UNIQUE (session_id, bank_line_id)
);
CREATE INDEX IF NOT EXISTS idx_rec_lines_session ON rec_lines(session_id);
CREATE INDEX IF NOT EXISTS idx_rec_lines_cleared ON rec_lines(session_id) WHERE cleared;

-- Session pages order uncleared lines by date; cover large statements.
CREATE INDEX IF NOT EXISTS idx_bsl_account_date
    ON bank_statement_lines(account_id, statement_date);
