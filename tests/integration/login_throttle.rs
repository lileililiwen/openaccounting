//! HTTP integration tests for the login rate limiter (per-account
//! and per-IP throttling, generic error pages, cooldown rollover,
//! audit row insertion).
//!
//! Each test gets a fresh `TestDb` so `login_attempts` starts empty.
//! All five requests come from `127.0.0.1`, which is what the
//! `ConnectInfo<SocketAddr>` extractor reports for every test
//! connection.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use time::OffsetDateTime;

/// The HTML body rendered for a generic wrong-password / throttled
/// login. Both responses are rendered from the same Askama template
/// with the same context, so the bodies are byte-equal. The spec
/// (`s2-login-rate-limiting`) requires this to avoid leaking
/// whether an email is valid.
fn assert_generic_login_body(body: &str) {
    assert!(
        body.contains("Invalid email or password"),
        "generic login body must contain 'Invalid email or password'; got: {}",
        &body[..body.len().min(200)]
    );
    assert!(
        body.contains(r#"<form method="post" action="/login""#),
        "body should be the login page (has the login form); got first 200 chars: {}",
        &body[..body.len().min(200)]
    );
}

async fn attempt(server: &TestServer, email: &str, password: &str) -> reqwest::Response {
    server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login POST")
}

async fn wipe_attempts(server: &TestServer) {
    let pool = server.db().pool();
    sqlx::query("DELETE FROM login_attempts")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn http_login_throttled_after_5_failures() {
    let server = TestServer::new().await;

    // Register a user so the email exists and Argon2id can fail
    // for the wrong password.
    let email = "alice@example.com";
    let correct = "correct horse battery staple";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "alice"),
            ("password", correct),
            ("password_confirm", correct),
        ])
        .send()
        .await
        .unwrap();
    wipe_attempts(&server).await;

    // Five wrong-password attempts. Each must be 200 (the login
    // page renders at 200; the throttle only kicks in on the 6th).
    for i in 0..5 {
        let resp = attempt(&server, email, "wrong").await;
        assert_eq!(
            resp.status(),
            200,
            "attempt #{i} should render the login page (200)"
        );
    }

    // The 6th attempt must be throttled (HTTP 429).
    let resp = attempt(&server, email, "wrong").await;
    assert_eq!(resp.status(), 429, "6th attempt must be throttled with 429");
    let body = resp.text().await.unwrap();
    assert_generic_login_body(&body);
}

#[tokio::test]
async fn http_login_throttle_indistinguishable_from_wrong_password() {
    let server = TestServer::new().await;

    let email = "bob@example.com";
    let correct = "correct horse battery staple";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "bob"),
            ("password", correct),
            ("password_confirm", correct),
        ])
        .send()
        .await
        .unwrap();
    wipe_attempts(&server).await;

    // Capture the wrong-password body (1 failure only — not yet throttled).
    let resp = attempt(&server, email, "wrong1").await;
    assert_eq!(resp.status(), 200);
    let wrong_body = resp.text().await.unwrap();
    assert_generic_login_body(&wrong_body);

    // Drive the account to throttled state.
    for _ in 0..4 {
        let _ = attempt(&server, email, "wrong").await;
    }
    // Capture the throttled body.
    let resp = attempt(&server, email, "wrong").await;
    assert_eq!(resp.status(), 429);
    let throttled_body = resp.text().await.unwrap();

    // Bodies must be byte-equal.
    assert_eq!(
        wrong_body, throttled_body,
        "throttled body must be byte-equal to wrong-password body"
    );
}

#[tokio::test]
async fn http_login_success_resets_counter() {
    let server = TestServer::new().await;

    let email = "carol@example.com";
    let correct = "correct horse battery staple";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "carol"),
            ("password", correct),
            ("password_confirm", correct),
        ])
        .send()
        .await
        .unwrap();
    wipe_attempts(&server).await;

    // 4 failures (still allowed).
    for _ in 0..4 {
        let resp = attempt(&server, email, "wrong").await;
        assert_eq!(resp.status(), 200);
    }

    // 1 successful login.
    let resp = attempt(&server, email, correct).await;
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "success must return a redirect (303) or 200; got {}",
        resp.status()
    );

    // 4 more failures — still allowed (counter was reset).
    for i in 0..4 {
        let resp = attempt(&server, email, "wrong").await;
        assert_eq!(
            resp.status(),
            200,
            "post-success failure #{i} must be allowed"
        );
    }

    // The 5th post-success failure crosses the line; the NEXT attempt
    // is throttled.
    let _ = attempt(&server, email, "wrong").await;
    let resp = attempt(&server, email, "wrong").await;
    assert_eq!(
        resp.status(),
        429,
        "after 5 fresh failures, the next attempt must be 429"
    );
}

#[tokio::test]
async fn http_login_ip_throttled_after_20_failures() {
    let server = TestServer::new().await;

    wipe_attempts(&server).await;

    // 20 failures across 5 different emails (4 each) so the
    // per-account throttle never trips. All requests come from
    // 127.0.0.1 (loopback), so they share the IP bucket.
    for i in 0..5 {
        let email = format!("u{i}@example.com");
        // Register each so Argon2id runs and returns 200 on wrong pw
        // (the account doesn't need to exist for IP counting — but
        // registering also exercises the path).
        let correct = "correct horse battery staple";
        let username = format!("u{i}");
        let resp = server
            .client()
            .post(format!("{}/register", server.base_url()))
            .form(&[
                ("email", email.as_str()),
                ("username", username.as_str()),
                ("password", correct),
                ("password_confirm", correct),
            ])
            .send()
            .await
            .unwrap();
        let _ = resp.status();
        for _ in 0..4 {
            let r = attempt(&server, &email, "wrong").await;
            assert_eq!(r.status(), 200, "per-account 4 fails must be allowed");
        }
    }
    wipe_attempts(&server).await;

    // Now generate exactly 20 failures across 5 emails (4 each)
    // using NON-EXISTENT emails so the per-account counter stays
    // at 0 and only the IP counter matters. After the 4th failure
    // for an email, that email's per-account count is 4 — still
    // under the threshold of 5.
    for i in 0..5 {
        let email = format!("none{i}@example.com");
        for _ in 0..4 {
            let r = attempt(&server, &email, "wrong").await;
            assert_eq!(
                r.status(),
                200,
                "pre-throttle per-email failures should render 200"
            );
        }
    }

    // 21st attempt (against a 6th email) must be IP-throttled.
    let resp = attempt(&server, "noneyet@example.com", "wrong").await;
    assert_eq!(
        resp.status(),
        429,
        "21st failure from same IP must be throttled"
    );
}

#[tokio::test]
async fn http_login_cooldown_window_resets() {
    // We simulate the 10-minute cooldown by inserting backdated
    // failure rows directly into the DB. The 6th live attempt
    // should NOT be throttled because the old failures have rolled
    // out of the 10-min window.
    let server = TestServer::new().await;
    let pool = server.db().pool();

    sqlx::query("DELETE FROM login_attempts")
        .execute(&pool)
        .await
        .unwrap();

    let email = "dave@example.com";
    let now = OffsetDateTime::now_utc();
    // 5 failures 11 minutes ago — outside the window.
    for _ in 0..5 {
        sqlx::query(
            "INSERT INTO login_attempts (ip, email, success, ts) VALUES ($1::inet, $2, false, $3)",
        )
        .bind("127.0.0.1")
        .bind(email)
        .bind(now - time::Duration::minutes(11))
        .execute(&pool)
        .await
        .unwrap();
    }

    // A fresh attempt must NOT be throttled (failures are stale).
    let resp = attempt(&server, email, "wrong").await;
    assert_eq!(
        resp.status(),
        200,
        "failures outside the 10-min window must not throttle"
    );
}

#[tokio::test]
async fn http_login_records_success_and_failure_audit_rows() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let email = "eve@example.com";
    let correct = "correct horse battery staple";
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "eve"),
            ("password", correct),
            ("password_confirm", correct),
        ])
        .send()
        .await
        .unwrap();
    sqlx::query("DELETE FROM login_attempts")
        .execute(&pool)
        .await
        .unwrap();

    // Two failures (no success yet) must produce 2 failure rows
    // and 0 success rows. This covers the "failure recorded"
    // scenario in the spec without the success-resets counter
    // logic wiping the rows.
    let _ = attempt(&server, email, "wrong").await;
    let _ = attempt(&server, email, "wrong").await;

    let (fails,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM login_attempts WHERE success = FALSE")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (ok,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM login_attempts WHERE success = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        fails, 2,
        "exactly two failure rows after two wrong attempts"
    );
    assert_eq!(ok, 0, "no success rows yet");

    // Audit row contains the lowercased email.
    let (stored_email,): (String,) =
        sqlx::query_as("SELECT email FROM login_attempts WHERE success = FALSE LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_email, email);

    // Now succeed: per the spec, success resets the per-account
    // failure counter, which means the recent failure rows are
    // deleted. Only the success row remains.
    let _ = attempt(&server, email, correct).await;

    let (fails,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM login_attempts WHERE success = FALSE")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (ok,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM login_attempts WHERE success = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fails, 0, "success resets per-account failure counter");
    assert_eq!(ok, 1, "exactly one success row remains");

    // Cleanup
    sqlx::query("DELETE FROM login_attempts WHERE email = $1")
        .bind(email)
        .execute(&pool)
        .await
        .ok();
}
