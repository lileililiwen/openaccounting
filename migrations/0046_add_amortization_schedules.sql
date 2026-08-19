-- ============================================================================
-- 0046_add_amortization_schedules.sql — a10-amortization
-- ============================================================================
-- Auto-amortization schedules for intangible assets and deferred
-- revenue / expense. Each schedule has a source account (where
-- the unamortized balance lives), a target account (where the
-- amortized amount lands each period), a total amount, a
-- period unit, a count of periods, and a start date.
-- ============================================================================

CREATE TABLE IF NOT EXISTS amortization_schedules (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id           UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    description         TEXT NOT NULL,
    source_account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    target_account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    total_amount        NUMERIC(20,4) NOT NULL CHECK (total_amount >= 0),
    period_unit         TEXT NOT NULL
                            CHECK (period_unit IN ('monthly', 'quarterly', 'yearly')),
    periods             INTEGER NOT NULL CHECK (periods > 0),
    start_date          DATE NOT NULL,
    end_date            DATE NOT NULL,
    next_post_date      DATE NOT NULL,
    posted_periods      INTEGER NOT NULL DEFAULT 0,
    skipped_periods     INTEGER NOT NULL DEFAULT 0,
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_amort_ledger
    ON amortization_schedules(ledger_id);
CREATE INDEX IF NOT EXISTS idx_amort_due
    ON amortization_schedules(next_post_date) WHERE is_active;
CREATE INDEX IF NOT EXISTS idx_amort_active_ledger
    ON amortization_schedules(ledger_id) WHERE is_active;

-- Idempotency: ensure the same period isn't posted twice.
CREATE TABLE IF NOT EXISTS amortization_posted_periods (
    schedule_id         UUID NOT NULL
                            REFERENCES amortization_schedules(id) ON DELETE CASCADE,
    period_number       INTEGER NOT NULL CHECK (period_number > 0),
    posted_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (schedule_id, period_number)
);
