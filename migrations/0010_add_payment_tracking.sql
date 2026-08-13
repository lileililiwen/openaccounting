-- ============================================================================
-- 0010_add_payment_tracking.sql — Payment tracking with invoice application
-- ============================================================================

CREATE TABLE IF NOT EXISTS payments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    contact_id      UUID REFERENCES contacts(id) ON DELETE SET NULL,
    invoice_id      UUID REFERENCES invoices(id) ON DELETE SET NULL,
    transaction_id  UUID REFERENCES transactions(id) ON DELETE SET NULL,
    amount          NUMERIC(20,4) NOT NULL,
    payment_date    DATE NOT NULL,
    payment_method  TEXT NOT NULL CHECK (payment_method IN ('cash', 'check', 'bank_transfer', 'credit_card', 'other')),
    reference       TEXT,
    kind            TEXT NOT NULL CHECK (kind IN ('received', 'made')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_payments_ledger ON payments(ledger_id);
CREATE INDEX IF NOT EXISTS idx_payments_contact ON payments(contact_id);
CREATE INDEX IF NOT EXISTS idx_payments_invoice ON payments(invoice_id);
CREATE INDEX IF NOT EXISTS idx_payments_date ON payments(payment_date);
CREATE INDEX IF NOT EXISTS idx_payments_unapplied ON payments(ledger_id) WHERE invoice_id IS NULL;
