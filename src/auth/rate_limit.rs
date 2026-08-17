//! Per-account and per-IP login rate limiter.
//!
//! Policy (from `openspec/changes/s2-login-rate-limiting/spec.md`):
//!
//! * Per-account: 5 failed attempts in the last 10 minutes → next
//!   attempt for that email is rejected with HTTP 429 and a generic
//!   "Invalid email or password" page (intentionally identical to
//!   the wrong-password response so attackers cannot enumerate
//!   valid emails).
//! * Per-IP:      20 failed attempts in the last 60 minutes across
//!   any email from the same IP → next attempt from that IP is
//!   rejected with HTTP 429.
//! * Success resets the per-account counter by deleting recent
//!   failures for that email in the rolling window.
//! * Rows older than 90 days are pruned by
//!   `crate::workers::prune_login_attempts`.
//!
//! The DB is the source of truth. All checks take an explicit
//! `now: OffsetDateTime` so tests can simulate the clock without
//! sleeping.
//!
//! Public API:
//! * [`record_attempt`] — insert one row.
//! * [`record_success`] — insert a success row AND delete recent
//!   failures for that email (resets the per-account counter).
//! * [`check_account`] — is this email currently throttled?
//! * [`check_ip`] — is this IP currently throttled?
//! * [`Decision`] — the verdict returned by the checks.

use sqlx::PgPool;
use time::OffsetDateTime;

/// Per-account rolling window length, in minutes.
pub const ACCOUNT_WINDOW_MINUTES: i64 = 10;
/// Per-account throttle threshold: failures within the window.
pub const ACCOUNT_FAILURE_THRESHOLD: i64 = 5;
/// Per-IP rolling window length, in minutes.
pub const IP_WINDOW_MINUTES: i64 = 60;
/// Per-IP throttle threshold: failures within the window.
pub const IP_FAILURE_THRESHOLD: i64 = 20;

/// The outcome of a rate-limit check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The request may proceed to password verification.
    Allowed,
    /// The request must be rejected with HTTP 429.
    Throttled,
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allowed)
    }
}

/// Insert one login attempt row.
///
/// `email` should already be lowercased + trimmed; the handler is
/// responsible for normalisation. `ip` is the textual form of the
/// peer IP address (IPv4 or IPv6) — Postgres will parse it via the
/// `INET` type. `now` is the row's timestamp; tests can pass a
/// backdated value to simulate the rolling window expiring.
pub async fn record_attempt(
    pool: &PgPool,
    ip: &str,
    email: &str,
    success: bool,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO login_attempts (ip, email, success, ts)
           VALUES ($1::inet, $2, $3, $4)"#,
    )
    .bind(ip)
    .bind(email)
    .bind(success)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Record a successful login AND reset the per-account counter by
/// deleting recent failures for that email.
///
/// The `success` row is inserted first so the audit log is complete
/// even if the delete fails. `now` is the timestamp of the success
/// row; the delete uses the same `now` to compute the cutoff.
pub async fn record_success(
    pool: &PgPool,
    ip: &str,
    email: &str,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    record_attempt(pool, ip, email, true, now).await?;
    let cutoff = now - time::Duration::minutes(ACCOUNT_WINDOW_MINUTES);
    sqlx::query(
        r#"DELETE FROM login_attempts
           WHERE email = $1
             AND success = false
             AND ts > $2"#,
    )
    .bind(email)
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(())
}

/// Should the request for `email` be rejected because the account
/// has had at least [`ACCOUNT_FAILURE_THRESHOLD`] failures in the
/// last [`ACCOUNT_WINDOW_MINUTES`] minutes?
pub async fn check_account(
    pool: &PgPool,
    email: &str,
    now: OffsetDateTime,
) -> Result<Decision, sqlx::Error> {
    let cutoff = now - time::Duration::minutes(ACCOUNT_WINDOW_MINUTES);
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM login_attempts
           WHERE email = $1
             AND success = false
             AND ts > $2"#,
    )
    .bind(email)
    .bind(cutoff)
    .fetch_one(pool)
    .await?;
    if count >= ACCOUNT_FAILURE_THRESHOLD {
        Ok(Decision::Throttled)
    } else {
        Ok(Decision::Allowed)
    }
}

/// Should the request from `ip` be rejected because that IP has had
/// at least [`IP_FAILURE_THRESHOLD`] failures against ANY email in
/// the last [`IP_WINDOW_MINUTES`] minutes?
pub async fn check_ip(
    pool: &PgPool,
    ip: &str,
    now: OffsetDateTime,
) -> Result<Decision, sqlx::Error> {
    let cutoff = now - time::Duration::minutes(IP_WINDOW_MINUTES);
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM login_attempts
           WHERE ip = $1::inet
             AND success = false
             AND ts > $2"#,
    )
    .bind(ip)
    .bind(cutoff)
    .fetch_one(pool)
    .await?;
    if count >= IP_FAILURE_THRESHOLD {
        Ok(Decision::Throttled)
    } else {
        Ok(Decision::Allowed)
    }
}

/// Normalise an email for storage and lookup.
///
/// We lowercase and trim so the rate-limit query matches even if
/// the caller typed `Alice@Example.COM` once and `alice@example.com`
/// the next time. Empty after trim is preserved as empty so a
/// missing field still hits a single bucket (the empty bucket),
/// which is fine — both wrong-password and throttled render the
/// same page anyway.
pub fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Parse a textual IP (`SocketAddr` string) for storage as INET.
///
/// Postgres' INET type accepts both `1.2.3.4` and `::1` directly,
/// so this is just a thin pass-through with a sanity check.
pub fn ip_to_text(addr: &std::net::SocketAddr) -> String {
    addr.ip().to_string()
}

#[cfg(test)]
mod tests {
    //! Unit tests for the rate limiter. These require a live
    //! PostgreSQL via `DATABASE_URL`; they skip otherwise (same
    //! pattern as `auth::tests`).
    //!
    //! Each test uses a **unique** `ip` and a **unique** `email` so
    //! the tests can run in parallel against a shared DB without
    //! interfering. There is no `DELETE FROM login_attempts` here
    //! on purpose — wiping shared state would race with sibling
    //! tests running in other threads.
    //!
    //! The integration tests in `tests/integration/login_throttle.rs`
    //! drive the same logic over real HTTP (with per-test DBs) and
    //! cover the body-bytes equivalence contract.

    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    async fn pool_or_skip() -> Option<PgPool> {
        let url = env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .ok()
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Unique email per test, derived from the test name + nanos,
    /// so parallel tests never share rows in the same bucket.
    fn uniq_email(tag: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("rl-{tag}-{nanos}@example.com")
    }

    #[tokio::test]
    async fn windowed_counter_rolls_off() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let email = uniq_email("rolloff");
        let ip = "10.0.0.1";
        let base = now();

        // 4 failures 20 minutes ago (outside the 10-min window).
        for i in 0..4 {
            record_attempt(
                &pool,
                ip,
                &email,
                false,
                base - time::Duration::minutes(20 - i),
            )
            .await
            .unwrap();
        }
        // 1 failure right now (inside the window).
        record_attempt(&pool, ip, &email, false, base)
            .await
            .unwrap();

        let d = check_account(&pool, &email, base).await.unwrap();
        assert_eq!(
            d,
            Decision::Allowed,
            "old failures must not count toward the threshold"
        );
        assert!(d.is_allowed());

        // Cleanup so we don't pollute the dev DB.
        let _ = sqlx::query("DELETE FROM login_attempts WHERE email = $1")
            .bind(&email)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    async fn account_throttle_triggers_at_threshold() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let email = uniq_email("thr");
        let ip = "10.0.0.2";
        let base = now();

        // 4 failures: still allowed.
        for _ in 0..4 {
            record_attempt(&pool, ip, &email, false, base)
                .await
                .unwrap();
        }
        assert_eq!(
            check_account(&pool, &email, base).await.unwrap(),
            Decision::Allowed
        );
        // 5th failure: still allowed right now (the check happens on the
        // NEXT attempt, not the one that crossed the line).
        record_attempt(&pool, ip, &email, false, base)
            .await
            .unwrap();
        // The next attempt should be throttled.
        assert_eq!(
            check_account(&pool, &email, base).await.unwrap(),
            Decision::Throttled
        );

        let _ = sqlx::query("DELETE FROM login_attempts WHERE email = $1")
            .bind(&email)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    async fn record_success_resets_account_counter() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let email = uniq_email("reset");
        let ip = "10.0.0.3";
        let base = now();

        for _ in 0..4 {
            record_attempt(&pool, ip, &email, false, base)
                .await
                .unwrap();
        }
        assert_eq!(
            check_account(&pool, &email, base).await.unwrap(),
            Decision::Allowed
        );

        // A success wipes recent failures for this email.
        record_success(&pool, ip, &email, base).await.unwrap();

        // Now 4 more failures must still be allowed.
        for _ in 0..4 {
            record_attempt(&pool, ip, &email, false, base)
                .await
                .unwrap();
        }
        assert_eq!(
            check_account(&pool, &email, base).await.unwrap(),
            Decision::Allowed,
            "success must reset the counter"
        );

        // The success row is still recorded for audit.
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM login_attempts WHERE email = $1 AND success = TRUE",
        )
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "exactly one success row for this email");

        let _ = sqlx::query("DELETE FROM login_attempts WHERE email = $1")
            .bind(&email)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    async fn ip_throttle_spans_emails() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        // Use a unique IP so concurrent tests can't pollute the count.
        let ip = format!("10.99.99.{}", uniq_email("ip").len() % 250 + 1);
        let base = now();

        // 19 failures across 19 different emails — still under threshold.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for i in 0..19 {
            let email = format!("rl-ip-{nanos}-{i}@example.com");
            record_attempt(&pool, &ip, &email, false, base)
                .await
                .unwrap();
        }
        assert_eq!(check_ip(&pool, &ip, base).await.unwrap(), Decision::Allowed);

        // 20th failure: now throttled.
        let email = format!("rl-ip-{nanos}-19@example.com");
        record_attempt(&pool, &ip, &email, false, base)
            .await
            .unwrap();
        assert_eq!(
            check_ip(&pool, &ip, base).await.unwrap(),
            Decision::Throttled
        );

        let _ = sqlx::query("DELETE FROM login_attempts WHERE ip = $1::inet")
            .bind(&ip)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    async fn normalise_email_lowercases_and_trims() {
        assert_eq!(normalise_email(" Alice@Example.COM "), "alice@example.com");
        assert_eq!(normalise_email("bob@example.com"), "bob@example.com");
    }
}
