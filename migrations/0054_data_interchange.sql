-- ============================================================================
-- 0054_data_interchange.sql — Statement formats + payee learning
-- ============================================================================
-- `data-interchange`: OFX/QIF/CAMT/MT940 ingestion and a learned
-- payee→account suggestion store.
-- ============================================================================

-- Stable provider id for cross-format duplicate detection (OFX FITID,
-- CAMT AcctSvcrRef, MT940 :20:). QIF has none — fingerprint dedupe
-- happens in the importer instead.
ALTER TABLE bank_statement_lines ADD COLUMN IF NOT EXISTS external_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS uq_bsl_external
    ON bank_statement_lines (account_id, external_id)
    WHERE external_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS payee_aliases (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id        UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    alias_normalized TEXT NOT NULL,
    canonical_payee  TEXT NOT NULL,
    account_id       UUID REFERENCES accounts(id) ON DELETE SET NULL,
    hit_count        INT NOT NULL DEFAULT 1,
    last_used_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, alias_normalized)
);

CREATE INDEX IF NOT EXISTS idx_payee_aliases_ledger ON payee_aliases(ledger_id);
