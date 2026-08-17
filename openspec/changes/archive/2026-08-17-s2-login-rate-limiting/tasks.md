## 1. Testing

- [x] 1.1 HTTP: `http_login_throttled_after_5_failures` — 5 wrong attempts, 6th returns 429.
- [x] 1.2 HTTP: `http_login_throttle_indistinguishable_from_wrong_password` — body bytes are identical.
- [x] 1.3 HTTP: `http_login_success_resets_counter` — 4 failures, 1 success, then 4 more failures are allowed.
- [x] 1.4 HTTP: `http_login_ip_throttled_after_20_failures` — 20 failures across many emails from one IP, 21st returns 429.
- [x] 1.5 HTTP: `http_login_cooldown_window_resets` — sleep 11 min (test-only env), the next attempt is allowed (mock clock).
- [x] 1.6 Unit: `windowed_counter_rolls_off` — old timestamps don't count toward the threshold.
- [x] 1.7 Worker: `prune_deletes_old_attempts` — insert rows with backdated ts, run worker, assert deletion.

## 2. Implementation

- [x] 2.1 `migrations/0028_add_login_attempts.sql` — table `(id, ip INET, email TEXT, success BOOLEAN, ts TIMESTAMPTZ)` + indexes on (email, ts) and (ip, ts).
- [x] 2.2 `src/auth/rate_limit.rs` — `RateLimiter` with `record_attempt`, `check_account`, `check_ip`.
- [x] 2.3 `src/auth/handlers.rs::login_submit` — wrap with rate-limit check before Argon2id verify.
- [x] 2.4 `src/workers/prune_login_attempts.rs` — daily tokio task.
- [x] 2.5 `src/lib.rs` — spawn the prune worker at startup.
- [x] 2.6 Generic error template that returns the same body for throttle and wrong-password.

## 3. Validation

- [x] 3.1 `openspec validate s2-login-rate-limiting` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets --features test-support` clean (no new warnings from this change; pre-existing warnings in unrelated files remain).
- [x] 3.4 `cargo test --features test-support` green; new file adds 7 cases (6 HTTP + 5 unit + 1 worker unit).
- [x] 3.5 `openspec archive s2-login-rate-limiting`.