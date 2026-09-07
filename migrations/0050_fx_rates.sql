-- ============================================================================
-- 0050_fx_rates.sql — Dated FX rate store + foreign-amount postings
-- ============================================================================
-- `multi-currency-fx`: currency was previously a label only. This adds
--
--   1. `fx_rates` — one row per (base, quote, date). `rate` is the amount
--      of `quote_currency` per ONE unit of `base_currency`. The UNIQUE
--      constraint makes feed upserts idempotent (ON CONFLICT DO NOTHING)
--      while manual entry wins by upserting with source='manual'.
--   2. `postings.foreign_amount` / `foreign_currency` — optional original
--      currency leg; the base amount stays authoritative for the balance
--      trigger and every report.
--   3. `fx_revaluations` — month-end revaluation ledger used both as the
--      idempotency key (one revaluation per account per month) and as the
--      unrealized section of the FX gains report.
-- ============================================================================

CREATE TABLE IF NOT EXISTS fx_rates (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    base_currency  CHAR(3) NOT NULL,
    quote_currency CHAR(3) NOT NULL,
    rate           NUMERIC(24,12) NOT NULL CHECK (rate > 0),
    rate_date      DATE NOT NULL,
    source         TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'ecb')),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (base_currency, quote_currency, rate_date)
);

CREATE INDEX IF NOT EXISTS idx_fx_rates_lookup
    ON fx_rates (base_currency, quote_currency, rate_date DESC);

ALTER TABLE postings ADD COLUMN IF NOT EXISTS foreign_amount NUMERIC(20,4);
ALTER TABLE postings ADD COLUMN IF NOT EXISTS foreign_currency CHAR(3);

CREATE TABLE IF NOT EXISTS fx_revaluations (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id      UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    account_id     UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    period_month   DATE NOT NULL,
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    fx_gain_loss   NUMERIC(20,4) NOT NULL,
    posted_by      UUID NOT NULL REFERENCES users(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, account_id, period_month)
);
