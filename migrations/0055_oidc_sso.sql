-- ============================================================================
-- 0055_oidc_sso.sql — OIDC identities + provider configuration
-- ============================================================================

-- Link between a local user and an IdP subject. (issuer, subject) is
-- the security identity; verified email is the linking convenience.
CREATE TABLE IF NOT EXISTS oidc_identities (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer     TEXT NOT NULL,
    subject    TEXT NOT NULL,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (issuer, subject)
);

CREATE INDEX IF NOT EXISTS idx_oidc_identities_user ON oidc_identities(user_id);

-- Single-provider configuration (one row). The client secret is
-- stored AES-256-GCM encrypted with the app-secret-derived key.
CREATE TABLE IF NOT EXISTS oidc_provider (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_url         TEXT NOT NULL,
    client_id          TEXT NOT NULL,
    client_secret_enc  TEXT NOT NULL,
    scopes             TEXT NOT NULL DEFAULT 'openid email profile',
    -- 'auto' provisions unknown emails; 'invite-only' rejects them.
    provisioning       TEXT NOT NULL DEFAULT 'invite-only'
                       CHECK (provisioning IN ('auto', 'invite-only')),
    -- SSO-only mode: password login + registration disabled instance-wide.
    sso_only           BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by         UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
