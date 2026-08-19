-- ============================================================================
-- 0039_add_api_tokens.sql — Per-user bearer tokens for the REST API
-- (`a1-rest-api`).
-- ============================================================================
--
-- The plaintext token is shown ONCE at creation and never
-- stored. The `token_hash` column holds an Argon2id hash of the
-- random portion of the token. The `token_prefix` is a short,
-- non-secret slice used for lookup (we index on it).
--
-- Tokens can be revoked (`revoked_at IS NOT NULL`) but are kept
-- in the table for audit purposes.

CREATE TABLE IF NOT EXISTS api_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,                -- user-supplied label
    token_prefix    TEXT NOT NULL UNIQUE,         -- first 12 chars after `oa_live_`
    token_hash      TEXT NOT NULL,                -- Argon2id hash of the secret portion
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at    TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);
