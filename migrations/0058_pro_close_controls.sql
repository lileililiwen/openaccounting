-- ============================================================================
-- 0058_pro_close_controls.sql — Professional period close and controls
-- (`pro-close-controls`): hard-close watermark, reopen audit, maker-checker,
-- accountant/auditor roles, gapless invoice numbering.
-- ============================================================================

-- Hard-close watermark: date-level. Legacy `period_year` rows stay valid;
-- new closes write both columns.
ALTER TABLE closed_periods ADD COLUMN IF NOT EXISTS closed_through DATE;
UPDATE closed_periods SET closed_through = make_date(period_year, 12, 31)
    WHERE closed_through IS NULL;
CREATE INDEX IF NOT EXISTS idx_closed_periods_through
    ON closed_periods(ledger_id, closed_through);

-- Reopen / override audit: every exception is attributable.
CREATE TABLE IF NOT EXISTS reopen_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    closed_through  DATE,
    reopened_by     UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    reason          TEXT NOT NULL CHECK (char_length(reason) >= 10),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reopen_events_ledger ON reopen_events(ledger_id);

-- Maker-checker approvals for over-threshold journals.
CREATE TABLE IF NOT EXISTS journal_approvals (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txn_id      UUID NOT NULL UNIQUE REFERENCES transactions(id) ON DELETE CASCADE,
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    maker       UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    checker     UUID REFERENCES users(id) ON DELETE SET NULL,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'approved', 'rejected')),
    amount      NUMERIC(20,4) NOT NULL CHECK (amount >= 0),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_journal_approvals_ledger ON journal_approvals(ledger_id, status);

-- Per-ledger approval threshold (base currency). Default 10,000; NULL or
-- negative disables maker-checker for solo/personal ledgers.
ALTER TABLE ledgers ADD COLUMN IF NOT EXISTS approval_threshold NUMERIC(20,4)
    NOT NULL DEFAULT 10000;

-- Gapless per-ledger-per-year invoice sequence counter.
CREATE TABLE IF NOT EXISTS invoice_sequences (
    ledger_id   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    year        INTEGER NOT NULL,
    last_no     INTEGER NOT NULL DEFAULT 0 CHECK (last_no >= 0),
    PRIMARY KEY (ledger_id, year)
);

-- Invoice void-with-reason. `invoice_number` stays the human-citable
-- identity; voids retain their number.
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS void_reason TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS uq_invoices_number
    ON invoices (ledger_id, invoice_number)
    WHERE invoice_number IS NOT NULL AND invoice_number <> '';

-- Widen transactions.kind with 'pending' (maker-checker holding state,
-- excluded from reports like 'draft').
DO $$
DECLARE
    constraint_name TEXT;
BEGIN
    SELECT con.conname INTO constraint_name
    FROM pg_constraint con
    JOIN pg_class rel ON rel.oid = con.conrelid
    WHERE rel.relname = 'transactions'
      AND con.contype = 'c'
      AND pg_get_constraintdef(con.oid) LIKE '%kind%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE transactions DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;
ALTER TABLE transactions ADD CONSTRAINT transactions_kind_check
    CHECK (kind IN ('standard', 'adjusting', 'closing', 'reversing',
                    'recurring', 'draft', 'posted', 'amortization', 'pending'));

-- Widen ledger role enums with accountant/auditor.
DO $$
DECLARE
    c TEXT;
BEGIN
    FOR c IN SELECT con.conname FROM pg_constraint con
             JOIN pg_class rel ON rel.oid = con.conrelid
             WHERE rel.relname = 'ledger_members' AND con.contype = 'c'
               AND pg_get_constraintdef(con.oid) LIKE '%editor%'
    LOOP
        EXECUTE format('ALTER TABLE ledger_members DROP CONSTRAINT %I', c);
    END LOOP;
    FOR c IN SELECT con.conname FROM pg_constraint con
             JOIN pg_class rel ON rel.oid = con.conrelid
             WHERE rel.relname = 'ledger_invitations' AND con.contype = 'c'
               AND pg_get_constraintdef(con.oid) LIKE '%editor%'
    LOOP
        EXECUTE format('ALTER TABLE ledger_invitations DROP CONSTRAINT %I', c);
    END LOOP;
END $$;
ALTER TABLE ledger_members ADD CONSTRAINT ledger_members_role_check
    CHECK (role IN ('editor', 'viewer', 'accountant', 'auditor'));
ALTER TABLE ledger_invitations ADD CONSTRAINT ledger_invitations_role_check
    CHECK (role IN ('editor', 'viewer', 'accountant', 'auditor'));
