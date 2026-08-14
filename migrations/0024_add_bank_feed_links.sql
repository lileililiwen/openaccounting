-- Migration 0024: Bank feed links table for live transaction sync.
-- Stores encrypted provider access tokens; plaintext never written to DB.

CREATE TABLE bank_feed_links (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id                   UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    provider                    TEXT NOT NULL CHECK (provider IN ('plaid','gocardless','salt_edge','simplefin','manual')),
    institution_id              TEXT,
    account_id_at_provider      TEXT,
    account_id_in_ledger        UUID REFERENCES accounts(id) ON DELETE SET NULL,
    status                      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','disconnected','error')),
    access_token_encrypted      TEXT,
    refresh_token_encrypted     TEXT,
    cursor                      TEXT,
    last_synced_at              TIMESTAMPTZ,
    error_message               TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_bank_feed_links_ledger ON bank_feed_links (ledger_id);
CREATE INDEX idx_bank_feed_links_status ON bank_feed_links (status);

-- Deduplication table: tracks which provider txn IDs have been imported.
CREATE TABLE bank_feed_transactions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    link_id             UUID NOT NULL REFERENCES bank_feed_links(id) ON DELETE CASCADE,
    provider_txn_id     TEXT NOT NULL,
    transaction_id      UUID REFERENCES transactions(id) ON DELETE SET NULL,
    imported_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_bank_feed_txn UNIQUE (link_id, provider_txn_id)
);

CREATE INDEX idx_bank_feed_txn_link ON bank_feed_transactions (link_id);
