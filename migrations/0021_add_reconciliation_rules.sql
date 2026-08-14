-- ============================================================================
-- 0021_add_reconciliation_rules.sql — Rule-based auto-match / categorize
-- ============================================================================
--
-- Each rule has a JSONB predicate (the "when" — payee glob, amount range,
-- description glob, currency) and a JSONB action (the "then" — link to a
-- posting, set a GL account, flag for review). Priority is a small int;
-- lower wins. The evaluate_predicate Rust helper AND-combines every
-- key in the predicate; the handler picks the first matching rule by
-- priority.
-- ============================================================================

CREATE TABLE IF NOT EXISTS reconciliation_rules (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id    UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL
        CHECK (kind IN ('match', 'categorize', 'flag')),
    priority     INT NOT NULL DEFAULT 100,
    predicate    JSONB NOT NULL DEFAULT '{}'::jsonb,
    action       JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_recon_rules_ledger_priority
    ON reconciliation_rules (ledger_id, priority, is_active);
