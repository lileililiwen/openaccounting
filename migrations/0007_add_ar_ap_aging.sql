-- ============================================================================
-- 0007_add_ar_ap_aging.sql — Contacts, invoices, AR/AP aging
-- ============================================================================

-- Contacts table for customers and vendors
CREATE TABLE IF NOT EXISTS contacts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    email           TEXT,
    phone           TEXT,
    kind            TEXT NOT NULL CHECK (kind IN ('customer', 'vendor', 'both')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, name)
);
CREATE INDEX IF NOT EXISTS idx_contacts_ledger ON contacts(ledger_id);

-- Invoices table for AR/AP tracking
CREATE TABLE IF NOT EXISTS invoices (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    contact_id      UUID NOT NULL REFERENCES contacts(id) ON DELETE RESTRICT,
    kind            TEXT NOT NULL CHECK (kind IN ('receivable', 'payable')),
    invoice_number  TEXT,
    invoice_date    DATE NOT NULL,
    due_date        DATE NOT NULL,
    total           NUMERIC(20,4) NOT NULL,
    amount_paid     NUMERIC(20,4) NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'paid', 'overdue', 'void')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_invoices_ledger ON invoices(ledger_id);
CREATE INDEX IF NOT EXISTS idx_invoices_contact ON invoices(contact_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);

-- Add contact_id to transactions (nullable for backward compatibility)
ALTER TABLE transactions ADD COLUMN contact_id UUID REFERENCES contacts(id) ON DELETE SET NULL;

-- Add invoice_id to transactions for linking payments to invoices
ALTER TABLE transactions ADD COLUMN invoice_id UUID REFERENCES invoices(id) ON DELETE SET NULL;
