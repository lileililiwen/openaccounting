# ## Context

`src/auth/handlers.rs:53-80` is a stateless handler that calls
`password::verify_password` on every request. Argon2id default parameters
yield ~50-200 ms per verify on commodity hardware, so a single attacker can
sustain ~5-10 attempts/sec. Combined with leaked password lists, this is
sufficient to compromise weak passwords in hours.

## Goals / Non-Goals

**Goals:**
- Block credential stuffing / online brute force without inconveniencing
  legitimate users who fat-finger their password.
- Keep the response identical for "wrong password" and "throttled" so
  attackers cannot enumerate valid emails.

**Non-Goals:**
- Account-level permanent lockout (resets via email).
- CAPTCHA (separate change — S11 if needed).

## Decisions

- **Per-account counter is windowed**: store `(email, ts)` for failures,
  count those in the last 10 minutes / 60 minutes in SQL.
- **Per-IP counter** uses a Postgres `login_attempts(ip, ts)` table; the
  per-IP query is a single `SELECT count(*) ... WHERE ip = $1 AND ts > now()
  - interval '1 hour'`.
- **In-process cache** (DashMap) caches counters for 60 s to avoid DB load;
  authoritative answer always comes from the DB.
- **Generic response** is identical for "wrong password", "no such user",
  and "throttled". The handler returns `Invalid email or password` for all
  three.

## Risks / Trade-offs

- **Distributed deploys**: with multiple replicas, the in-process cache
  gives per-instance throttling. The DB-backed counter is the source of
  truth. A misconfigured attacker rotating IPs could bypass the IP throttle
  but not the account throttle.
- **Legitimate users behind NAT**: a corporate proxy may share an IP. The
  account-level throttle catches that case; the IP throttle is a backstop.
