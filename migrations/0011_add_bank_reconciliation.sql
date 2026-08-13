-- ============================================================================
-- 0011_add_bank_reconciliation.sql — Bank statement reconciliation
-- ============================================================================

CREATE TABLE IF NOT EXISTS bank_statement_lines (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id               UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    account_id              UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    statement_date          DATE NOT NULL,
    description             TEXT,
    amount                  NUMERIC(20,4) NOT NULL,
    check_number            TEXT,
    status                  TEXT NOT NULL DEFAULT 'unmatched' CHECK (status IN ('unmatched', 'matched', 'excluded')),
    matched_transaction_id  UUID REFERENCES transactions(id) ON DELETE SET NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bsl_account ON bank_statement_lines(account_id);
CREATE INDEX IF NOT EXISTS idx_bsl_status ON bank_statement_lines(status) WHERE status = 'unmatched';
CREATE INDEX IF NOT EXISTS idx_bsl_ledger ON bank_statement_lines(ledger_id);

CREATE TABLE IF NOT EXISTS reconciliations (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id           UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    account_id          UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    statement_date      DATE NOT NULL,
    statement_balance   NUMERIC(20,4),
    ledger_balance      NUMERIC(20,4),
    difference          NUMERIC(20,4),
    completed_by        UUID REFERENCES users(id) ON DELETE SET NULL,
    completed_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_recon_account ON reconciliations(account_id);

ALTER TABLE transactions ADD COLUMN IF NOT EXISTS is_reconciled BOOLEAN NOT NULL DEFAULT FALSE;
