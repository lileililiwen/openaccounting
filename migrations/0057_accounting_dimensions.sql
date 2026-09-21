-- ============================================================================
-- 0057_accounting_dimensions.sql — Dimensions, recurring journals, FX audit
-- ============================================================================
-- `accounting-dimensions`: posting-level cost-center/project dimensions,
-- ledger inventory valuation method, recurring journal templates with
-- idempotent per-period runs, and a manual FX override audit trail.

-- Ledger-scoped analysis dimensions. Names are unique per ledger.
CREATE TABLE IF NOT EXISTS cost_centers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, name)
);
CREATE TABLE IF NOT EXISTS projects (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, name)
);

-- Posting-level dimensions: nullable so all existing postings stay valid.
ALTER TABLE postings ADD COLUMN IF NOT EXISTS cost_center_id UUID REFERENCES cost_centers(id) ON DELETE SET NULL;
ALTER TABLE postings ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_postings_cost_center ON postings(cost_center_id) WHERE cost_center_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_postings_project ON postings(project_id) WHERE project_id IS NOT NULL;

-- Inventory valuation method per ledger (default keeps current behavior).
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS inventory_method TEXT NOT NULL DEFAULT 'average'
    CHECK (inventory_method IN ('fifo', 'average'));

-- Recurring journal templates. `next_period` is the first YYYY-MM-DD of
-- the next period to generate; runs are idempotent per (template, period).
CREATE TABLE IF NOT EXISTS recurring_journal_templates (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id           UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    frequency           TEXT NOT NULL DEFAULT 'monthly'
                        CHECK (frequency IN ('weekly', 'monthly', 'quarterly', 'yearly')),
    start_date          DATE NOT NULL,
    end_date            DATE,
    max_occurrences     INT CHECK (max_occurrences IS NULL OR max_occurrences > 0),
    is_paused           BOOLEAN NOT NULL DEFAULT FALSE,
    next_period         DATE NOT NULL,
    created_by          UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_rj_templates_ledger ON recurring_journal_templates(ledger_id);

CREATE TABLE IF NOT EXISTS recurring_journal_lines (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id     UUID NOT NULL REFERENCES recurring_journal_templates(id) ON DELETE CASCADE,
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    direction       TEXT NOT NULL CHECK (direction IN ('DEBIT', 'CREDIT')),
    amount          NUMERIC(20,4) NOT NULL CHECK (amount > 0),
    memo            TEXT,
    cost_center_id  UUID REFERENCES cost_centers(id) ON DELETE SET NULL,
    project_id      UUID REFERENCES projects(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_rj_lines_template ON recurring_journal_lines(template_id);

CREATE TABLE IF NOT EXISTS recurring_journal_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id     UUID NOT NULL REFERENCES recurring_journal_templates(id) ON DELETE CASCADE,
    period_key      TEXT NOT NULL,
    draft_txn_id    UUID REFERENCES transactions(id) ON DELETE SET NULL,
    posted_txn_id   UUID REFERENCES transactions(id) ON DELETE SET NULL,
    status          TEXT NOT NULL DEFAULT 'preview'
                    CHECK (status IN ('preview', 'posted', 'skipped')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (template_id, period_key)
);
CREATE INDEX IF NOT EXISTS idx_rj_runs_template ON recurring_journal_runs(template_id);

-- Manual FX override audit: who/when/old/new/reason for every manual
-- rate entry or edit. Lookup precedence is unchanged (see fx::lookup).
CREATE TABLE IF NOT EXISTS fx_override_audit (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    actor_id        UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    base_currency   CHAR(3) NOT NULL,
    quote_currency  CHAR(3) NOT NULL,
    rate_date       DATE NOT NULL,
    old_rate        NUMERIC(24,12),
    new_rate        NUMERIC(24,12) NOT NULL,
    reason          TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_fx_override_ledger ON fx_override_audit(ledger_id, rate_date);
