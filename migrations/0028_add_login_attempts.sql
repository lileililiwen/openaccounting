-- Migration 0028: Login attempts audit table for rate limiting.
--
-- Records every login attempt (success and failure) so the rate
-- limiter can count failures in rolling time windows:
--   * per-account: 5 failures in the last 10 minutes
--   * per-IP:      20 failures in the last 60 minutes (any email)
--
-- The `ip` column uses the INET type so PostgreSQL can index and
-- compare it efficiently; we accept both IPv4 and IPv6 from the
-- axum `ConnectInfo<SocketAddr>` extractor. `email` is stored
-- lowercased so the rate-limit query can do simple equality
-- without re-applying LOWER() on every call.
--
-- Old rows are deleted by `src/workers/prune_login_attempts.rs`
-- after 90 days (per `s2-login-rate-limiting` spec).

CREATE TABLE IF NOT EXISTS login_attempts (
    id      BIGSERIAL PRIMARY KEY,
    ip      INET NOT NULL,
    email   TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    ts      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Per-account throttle: "5 failures for email@example.com in last 10 min".
CREATE INDEX IF NOT EXISTS idx_login_attempts_email_ts
    ON login_attempts (email, ts);

-- Per-IP throttle: "20 failures from 1.2.3.4 in last 60 min across any email".
CREATE INDEX IF NOT EXISTS idx_login_attempts_ip_ts
    ON login_attempts (ip, ts);

-- Helps the daily prune worker sweep expired rows.
CREATE INDEX IF NOT EXISTS idx_login_attempts_ts
    ON login_attempts (ts);