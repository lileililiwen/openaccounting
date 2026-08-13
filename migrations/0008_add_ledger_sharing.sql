-- ============================================================================
-- 0008_add_ledger_sharing.sql — Multi-user collaboration with roles
-- ============================================================================

-- Ledger invitations
CREATE TABLE IF NOT EXISTS ledger_invitations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    inviter_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invitee_email   TEXT NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('editor', 'viewer')),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'declined', 'revoked', 'expired')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '7 days'),
    UNIQUE (ledger_id, invitee_email)
);
CREATE INDEX IF NOT EXISTS idx_invitations_ledger ON ledger_invitations(ledger_id);
CREATE INDEX IF NOT EXISTS idx_invitations_email ON ledger_invitations(invitee_email);

-- Ledger members (accepted invitations)
CREATE TABLE IF NOT EXISTS ledger_members (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ledger_id       UUID NOT NULL REFERENCES ledgers(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('editor', 'viewer')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (ledger_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_members_ledger ON ledger_members(ledger_id);
CREATE INDEX IF NOT EXISTS idx_members_user ON ledger_members(user_id);
