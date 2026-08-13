-- ============================================================================
-- 0009_add_recurring_transactions.sql — Recurring transaction templates
-- ============================================================================

CREATE TABLE IF NOT EXISTS transaction_templates (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    description     TEXT NOT NULL,
    payee           TEXT,
    reference       TEXT,
    frequency       TEXT NOT NULL CHECK (frequency IN ('weekly', 'biweekly', 'monthly', 'quarterly', 'yearly')),
    next_date       DATE NOT NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_templates_ledger ON transaction_templates(ledger_id);
CREATE INDEX IF NOT EXISTS idx_templates_due ON transaction_templates(next_date) WHERE is_active;

CREATE TABLE IF NOT EXISTS template_postings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id     UUID NOT NULL REFERENCES transaction_templates(id) ON DELETE CASCADE,
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    direction       TEXT NOT NULL CHECK (direction IN ('DEBIT', 'CREDIT')),
    amount          NUMERIC(20,4) NOT NULL,
    memo            TEXT
);
CREATE INDEX IF NOT EXISTS idx_template_postings_template ON template_postings(template_id);

ALTER TABLE transactions ADD COLUMN IF NOT EXISTS template_id UUID REFERENCES transaction_templates(id) ON DELETE SET NULL;
