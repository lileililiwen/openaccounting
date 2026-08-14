-- ============================================================================
-- 0019_add_reimbursement.sql — Expense reimbursement module
-- ============================================================================
--
-- Three tables:
--   reimbursement_claims   — the claim entity (one per request)
--   reimbursement_lines    — line items (one per receipt / expense)
--   reimbursement_events   — state-transition log (audit trail)
--
-- Account subtypes extended with two new variants:
--   employee_payable  (LIABILITY) — the company's open balance to employees
--   employee_advance  (ASSET)     — cash already paid out as a pre-payment
-- ============================================================================

CREATE TABLE IF NOT EXISTS reimbursement_claims (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id        UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    short_id         TEXT NOT NULL,
    employee_id      UUID NOT NULL REFERENCES users(id),
    employee_name    TEXT NOT NULL,
    title            TEXT NOT NULL,
    description      TEXT,
    currency         CHAR(3) NOT NULL,
    status           TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'approved', 'rejected', 'paid')),
    approved_by      UUID REFERENCES users(id),
    approved_at      TIMESTAMPTZ,
    rejected_reason  TEXT,
    paid_at          TIMESTAMPTZ,
    payout_account_id UUID REFERENCES accounts(id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, short_id)
);
CREATE INDEX IF NOT EXISTS idx_reimb_claims_ledger_status
    ON reimbursement_claims (ledger_id, status);

CREATE TABLE IF NOT EXISTS reimbursement_lines (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id         UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    txn_date         DATE NOT NULL,
    description      TEXT NOT NULL,
    amount           NUMERIC(20,4) NOT NULL CHECK (amount > 0),
    gl_account_id    UUID NOT NULL REFERENCES accounts(id),
    tax_amount       NUMERIC(20,4) NOT NULL DEFAULT 0 CHECK (tax_amount >= 0),
    advance_amount   NUMERIC(20,4) NOT NULL DEFAULT 0 CHECK (advance_amount >= 0),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reimb_lines_claim ON reimbursement_lines(claim_id);

CREATE TABLE IF NOT EXISTS reimbursement_events (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id         UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    actor_id         UUID NOT NULL REFERENCES users(id),
    event_type       TEXT NOT NULL,
    payload          JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reimb_events_claim ON reimbursement_events(claim_id, created_at);

-- Extend account subtype CHECK constraint with the two new
-- variants. The original constraint is rewritten to allow
-- the new subtypes while keeping every existing pair
-- allowed.
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS chk_account_subtype;
ALTER TABLE accounts ADD CONSTRAINT chk_account_subtype CHECK (
    (type = 'ASSET'     AND subtype IN ('CURRENT_ASSET', 'FIXED_ASSET', 'INTANGIBLE_ASSET', 'OTHER_ASSET', 'EMPLOYEE_ADVANCE')) OR
    (type = 'LIABILITY' AND subtype IN ('CURRENT_LIABILITY', 'LONG_TERM_LIABILITY', 'EMPLOYEE_PAYABLE')) OR
    (type = 'EQUITY'    AND subtype IN ('EQUITY', 'RETAINED_EARNINGS', 'DRAWING')) OR
    (type = 'INCOME'    AND subtype IN ('OPERATING_INCOME', 'NON_OPERATING_INCOME')) OR
    (type = 'EXPENSE'   AND subtype IN ('COST_OF_GOODS_SOLD', 'OPERATING_EXPENSE', 'NON_OPERATING_EXPENSE', 'TAX_EXPENSE'))
);
