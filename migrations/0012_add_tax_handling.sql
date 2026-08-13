-- ============================================================================
-- 0012_add_tax_handling.sql — Tax rates and tax liabilities
-- ============================================================================

CREATE TABLE IF NOT EXISTS tax_rates (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    rate            NUMERIC(6,4) NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('sales_tax', 'purchase_tax')),
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_tax_rates_ledger ON tax_rates(ledger_id);

CREATE TABLE IF NOT EXISTS posting_taxes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    posting_id      UUID NOT NULL REFERENCES postings(id) ON DELETE CASCADE,
    tax_rate_id     UUID NOT NULL REFERENCES tax_rates(id) ON DELETE CASCADE,
    tax_amount      NUMERIC(20,4) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (posting_id, tax_rate_id)
);
CREATE INDEX IF NOT EXISTS idx_posting_taxes_posting ON posting_taxes(posting_id);
CREATE INDEX IF NOT EXISTS idx_posting_taxes_rate ON posting_taxes(tax_rate_id);
