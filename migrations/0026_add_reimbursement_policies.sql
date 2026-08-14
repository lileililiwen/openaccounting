-- ============================================================================
-- 0026_add_reimbursement_policies.sql — Reimbursement policy engine
-- ============================================================================
--
-- Policies are config-driven rules that the reimbursement
-- handlers evaluate on line add / submit / approve. Each
-- policy is one of three kinds:
--   category_cap      — max amount per category per day
--   receipt_required  — lines at or above N require a receipt
--   per_diem          — destination-based daily allowance
--
-- The evaluator returns a list of `Violation`s; the submit /
-- approve handlers surface 400 on hard violations and an
-- info banner on soft ones.
-- ============================================================================

CREATE TABLE IF NOT EXISTS reimbursement_policies (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id    UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL
        CHECK (kind IN ('category_cap', 'receipt_required', 'per_diem')),
    config       JSONB NOT NULL DEFAULT '{}'::jsonb,
    severity     TEXT NOT NULL DEFAULT 'hard'
        CHECK (severity IN ('hard', 'soft')),
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_reimb_policies_ledger
    ON reimbursement_policies (ledger_id, is_active);
