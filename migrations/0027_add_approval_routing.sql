-- ============================================================================
-- 0027_add_approval_routing.sql — Multi-level / threshold-based approval routing
-- ============================================================================
--
-- Two new tables:
--   reimbursement_approval_policies — ledger-level thresholds mapping a
--     minimum claim total to a required approval level and role.
--   reimbursement_approval_steps    — recorded approvals per claim + level.
--
-- The reimbursement_claims.status CHECK is widened to admit the two new
-- state-machine statuses: 'partially_approved' and 'fully_approved'.
-- The existing 'approved' value stays valid for backward compatibility
-- (rows written before this migration keep their old status string).
-- ============================================================================

CREATE TABLE IF NOT EXISTS reimbursement_approval_policies (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    min_amount      NUMERIC(20,4) NOT NULL CHECK (min_amount >= 0),
    approver_role   TEXT NOT NULL CHECK (approver_role IN ('Admin','Accountant')),
    level           INT NOT NULL CHECK (level >= 1),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_reimb_approval_policies_ledger
    ON reimbursement_approval_policies (ledger_id, min_amount, level);

CREATE TABLE IF NOT EXISTS reimbursement_approval_steps (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id        UUID NOT NULL REFERENCES reimbursement_claims(id) ON DELETE CASCADE,
    level           INT NOT NULL,
    approver_id     UUID NOT NULL REFERENCES users(id),
    approved_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (claim_id, level, approver_id)
);

CREATE INDEX IF NOT EXISTS idx_reimb_approval_steps_claim
    ON reimbursement_approval_steps (claim_id, level);

-- Widen the status CHECK to admit the two new statuses. The old
-- constraint was created inline in migration 0019, so it carries
-- Postgres' auto-generated name.
ALTER TABLE reimbursement_claims
    DROP CONSTRAINT IF EXISTS reimbursement_claims_status_check;
ALTER TABLE reimbursement_claims
    ADD CONSTRAINT reimbursement_claims_status_check
    CHECK (status IN ('draft', 'submitted', 'partially_approved',
                      'fully_approved', 'approved', 'rejected', 'paid'));
