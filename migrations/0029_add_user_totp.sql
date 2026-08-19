-- Migration 0029: TOTP second-factor tables.
--
-- Two tables:
--   user_totp — exactly one row per enrolled user; the secret is
--     encrypted at rest with AES-256-GCM (key derived from APP_SECRET
--     via HKDF-SHA256, info="totp-secret-v1") and stored as
--     `<b64_nonce>:<b64_ciphertext>`. `last_used_counter` is the
--     30-second time-step counter of the most recently accepted
--     code; the verify path uses it for replay protection with a
--     ±1 step window for clock skew.
--
--   recovery_codes — ten one-time 8-character base32 codes per
--     user, Argon2id-hashed at rest. `consumed_at IS NULL` means
--     unused.
--
-- Both tables CASCADE-delete with the user.

CREATE TABLE IF NOT EXISTS user_totp (
    user_id              UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret_encrypted     TEXT NOT NULL,
    enrolled_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_counter    BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS recovery_codes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash   TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    consumed_at TIMESTAMPTZ
);

-- Fast lookup of unused codes during 2FA verification.
CREATE INDEX IF NOT EXISTS idx_recovery_codes_user_unused
    ON recovery_codes (user_id, consumed_at);
