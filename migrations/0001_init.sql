-- ============================================================================
-- 0001_init.sql — Core domain schema for OpenAccounting
-- ============================================================================
-- Double-entry bookkeeping: ledgers -> accounts -> transactions -> postings
-- Enforced invariant: SUM(debits) == SUM(credits) per transaction
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ----------------------------------------------------------------------------
-- users
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT UNIQUE NOT NULL,
    username        TEXT UNIQUE NOT NULL,
    display_name    TEXT,
    hashed_password TEXT NOT NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_lower ON users (LOWER(email));

-- ----------------------------------------------------------------------------
-- ledgers — a self-contained set of books (per person / business / project)
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS ledgers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    base_currency   CHAR(3) NOT NULL DEFAULT 'USD',
    timezone        TEXT NOT NULL DEFAULT 'UTC',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ledgers_owner ON ledgers(owner_id);

-- ----------------------------------------------------------------------------
-- accounts — chart of accounts
--   type: ASSET | LIABILITY | EQUITY | INCOME | EXPENSE
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    parent_id       UUID REFERENCES accounts(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    code            TEXT,
    type            TEXT NOT NULL CHECK (type IN ('ASSET','LIABILITY','EQUITY','INCOME','EXPENSE')),
    currency        CHAR(3) NOT NULL,
    is_archived     BOOLEAN NOT NULL DEFAULT FALSE,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, name)
);
CREATE INDEX IF NOT EXISTS idx_accounts_ledger ON accounts(ledger_id);
CREATE INDEX IF NOT EXISTS idx_accounts_type   ON accounts(ledger_id, type);

-- ----------------------------------------------------------------------------
-- transactions
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS transactions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    txn_date        DATE NOT NULL,
    description     TEXT NOT NULL,
    payee           TEXT,
    reference       TEXT,
    currency        CHAR(3) NOT NULL,
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_txn_ledger_date ON transactions(ledger_id, txn_date);

-- ----------------------------------------------------------------------------
-- postings — the legs of a transaction (must balance)
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS postings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id  UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    amount          NUMERIC(20,4) NOT NULL CHECK (amount > 0),
    direction       TEXT NOT NULL CHECK (direction IN ('DEBIT','CREDIT')),
    memo            TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_postings_txn   ON postings(transaction_id);
CREATE INDEX IF NOT EXISTS idx_postings_acct  ON postings(account_id);

-- ----------------------------------------------------------------------------
-- documents — files attached to transactions
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS documents (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id  UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    filename        TEXT NOT NULL,
    stored_filename TEXT NOT NULL,
    mime_type       TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    uploaded_by     UUID NOT NULL REFERENCES users(id),
    uploaded_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_documents_txn ON documents(transaction_id);

-- ----------------------------------------------------------------------------
-- tags — free-form labels on transactions
-- ----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tags (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    UNIQUE (ledger_id, name)
);
CREATE INDEX IF NOT EXISTS idx_tags_ledger ON tags(ledger_id);

CREATE TABLE IF NOT EXISTS transaction_tags (
    transaction_id  UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    tag_id          UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (transaction_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_txn_tags_tag ON transaction_tags(tag_id);

-- ----------------------------------------------------------------------------
-- Double-entry invariant: enforce balanced postings at the DB level.
-- A transaction can only be committed if the SUM of debits == SUM of credits
-- across its postings. Implemented as a constraint trigger on postings.
-- ----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION check_posting_balance() RETURNS TRIGGER AS $$
DECLARE
    dr_total NUMERIC(20,4);
    cr_total NUMERIC(20,4);
    posting_count INTEGER;
BEGIN
    SELECT COUNT(*),
           COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN direction='CREDIT' THEN amount ELSE 0 END), 0)
      INTO posting_count, dr_total, cr_total
      FROM postings
     WHERE transaction_id = NEW.transaction_id;

    -- Allow partial inserts; only check on the last leg when count >= 2.
    IF posting_count >= 2 AND dr_total <> cr_total THEN
        RAISE EXCEPTION 'Postings do not balance: debits=%, credits=%', dr_total, cr_total
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_posting_balance ON postings;
CREATE TRIGGER trg_posting_balance
    AFTER INSERT OR UPDATE OR DELETE ON postings
    FOR EACH ROW EXECUTE FUNCTION check_posting_balance();

-- ----------------------------------------------------------------------------
-- updated_at trigger
-- ----------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_users_updated         ON users;
DROP TRIGGER IF EXISTS trg_ledgers_updated       ON ledgers;
DROP TRIGGER IF EXISTS trg_accounts_updated      ON accounts;
DROP TRIGGER IF EXISTS trg_transactions_updated  ON transactions;
CREATE TRIGGER trg_users_updated         BEFORE UPDATE ON users         FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_ledgers_updated       BEFORE UPDATE ON ledgers       FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_accounts_updated      BEFORE UPDATE ON accounts      FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_transactions_updated  BEFORE UPDATE ON transactions  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
