-- ============================================================================
-- 0013_add_budgets.sql — Budget tracking and alerts
-- ============================================================================

CREATE TABLE IF NOT EXISTS budgets (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    period          TEXT NOT NULL CHECK (period IN ('monthly', 'quarterly', 'yearly')),
    amount          NUMERIC(20,4) NOT NULL,
    alert_threshold NUMERIC(5,4) NOT NULL DEFAULT 0.8,
    start_date      DATE NOT NULL,
    end_date        DATE NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_budgets_ledger ON budgets(ledger_id);
CREATE INDEX IF NOT EXISTS idx_budgets_account ON budgets(account_id);

CREATE TABLE IF NOT EXISTS budget_alerts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    budget_id       UUID NOT NULL REFERENCES budgets(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message         TEXT NOT NULL,
    threshold_pct   NUMERIC(5,4) NOT NULL,
    amount          NUMERIC(20,4) NOT NULL,
    budget_amount   NUMERIC(20,4) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    acknowledged_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_budget_alerts_user ON budget_alerts(user_id);
CREATE INDEX IF NOT EXISTS idx_budget_alerts_unack ON budget_alerts(user_id) WHERE acknowledged_at IS NULL;
