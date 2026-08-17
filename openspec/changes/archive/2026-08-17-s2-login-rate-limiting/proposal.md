# Add Login Rate Limiting and Account Lockout

## Why

`POST /login` (`src/auth/handlers.rs:53-80`) accepts unlimited Argon2id
verifications. Argon2id is intentionally slow (~50-200 ms), which slows
attackers but also lets a single attacker consume ~5-10 CPU-seconds per
attempt. There is no per-IP or per-account throttle and no lockout. OWASP
Authentication Cheat Sheet recommends both rate limiting and exponential
backoff / lockout. Firefly III ships Laravel `ThrottleRequests`. Akaunting
uses `cache`-backed counters.

## What Changes

- Add an in-memory + Postgres-backed rate limiter keyed on IP and on
  lowercased email.
- Policy: 5 failed attempts in 10 minutes → 1-minute cooldown; 20 failed in
  1 hour → 30-minute lockout with a generic "Too many attempts" page.
- Successful login resets the counters.
- A new `login_attempts` table records `(ip, email, success, ts)` for audit;
  after 90 days the rows are deleted by a daily worker.

## Capabilities

### New Capabilities

- `login-rate-limiting`: Throttle and lockout for failed logins.

## Impact

**New files:**
- `src/auth/rate_limit.rs`
- `migrations/0028_add_login_attempts.sql`
- `tests/http/login_throttle.rs`

**Modified files:**
- `src/auth/handlers.rs` — wrap login with limiter
- `src/workers/mod.rs` — add daily prune worker
- `src/lib.rs` — start the prune worker
