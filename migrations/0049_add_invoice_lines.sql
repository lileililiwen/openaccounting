-- ============================================================================
-- 0049_add_invoice_lines.sql — Invoice line items (`a18-invoicing-upgrade`)
-- ============================================================================

CREATE TABLE IF NOT EXISTS invoice_lines (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id  UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    quantity    NUMERIC(12,4) NOT NULL DEFAULT 1,
    unit_price  NUMERIC(20,4) NOT NULL DEFAULT 0,
    amount      NUMERIC(20,4) NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice ON invoice_lines(invoice_id);
