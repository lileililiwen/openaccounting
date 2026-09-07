-- ============================================================================
-- 0053_ar_getting_paid.sql — Estimates, recurring invoices, share links,
-- e-invoicing fields (`ar-getting-paid`)
-- ============================================================================

-- One document model for invoices AND estimates (`invoicing-completeness`).
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS doc_kind TEXT NOT NULL DEFAULT 'invoice';
ALTER TABLE invoices DROP CONSTRAINT IF EXISTS invoices_status_check;
ALTER TABLE invoices ADD CONSTRAINT invoices_status_check
    CHECK (status IN ('open', 'paid', 'overdue', 'void',
                      'draft', 'sent', 'accepted', 'declined', 'expired', 'converted'));

-- Recurring invoice templates.
CREATE TABLE IF NOT EXISTS recurring_invoice_templates (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id     UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    contact_id    UUID NOT NULL REFERENCES contacts(id) ON DELETE RESTRICT,
    kind          TEXT NOT NULL CHECK (kind IN ('receivable', 'payable')),
    frequency     TEXT NOT NULL CHECK (frequency IN ('weekly', 'biweekly', 'monthly', 'quarterly', 'yearly')),
    next_date     DATE NOT NULL,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS recurring_invoice_lines (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id    UUID NOT NULL REFERENCES recurring_invoice_templates(id) ON DELETE CASCADE,
    description    TEXT NOT NULL,
    quantity       NUMERIC(20,4) NOT NULL,
    unit_price     NUMERIC(20,4) NOT NULL,
    sort_order     INT NOT NULL DEFAULT 0
);

ALTER TABLE invoices ADD COLUMN IF NOT EXISTS recurring_template_id UUID
    REFERENCES recurring_invoice_templates(id) ON DELETE SET NULL;

-- Occurrence idempotency: one issued invoice per template per date.
CREATE UNIQUE INDEX IF NOT EXISTS uq_recurring_invoice_due
    ON invoices (recurring_template_id, invoice_date)
    WHERE recurring_template_id IS NOT NULL;

-- Public share links; tokens stored hashed (like API tokens).
CREATE TABLE IF NOT EXISTS invoice_shares (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id    UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    doc_kind     TEXT NOT NULL CHECK (doc_kind IN ('invoice', 'estimate')),
    invoice_id   UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    created_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    revoked_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_invoice_shares_doc ON invoice_shares(invoice_id);

-- EN 16931 fields for statutory e-invoicing (`e-invoicing-facturx`).
-- The SELLER identity lives on the ledger (one business per ledger's books);
-- the BUYER identity on the contact.
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS vat_id TEXT;
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS address_line TEXT;
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS city TEXT;
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS postal_code TEXT;
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS country_code CHAR(2);
ALTER TABLE contacts ADD COLUMN IF NOT EXISTS vat_id TEXT;
ALTER TABLE contacts ADD COLUMN IF NOT EXISTS address_line TEXT;
ALTER TABLE contacts ADD COLUMN IF NOT EXISTS city TEXT;
ALTER TABLE contacts ADD COLUMN IF NOT EXISTS postal_code TEXT;
ALTER TABLE contacts ADD COLUMN IF NOT EXISTS country_code CHAR(2);
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS payment_terms TEXT;
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS payment_means_code TEXT;
