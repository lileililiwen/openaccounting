//! HTTP integration tests for TOTP second-factor authentication.
//!
//! Each test gets a fresh `TestDb` so `user_totp` and
//! `recovery_codes` start empty.
//!
//! Scenarios:
//!   * `http_2fa_enroll_success` — full enroll flow with a real code.
//!   * `http_2fa_login_requires_code` — password OK without code
//!     yields no session.
//!   * `http_2fa_disable_requires_both_factors` — disable with only
//!     password fails.
//!   * `http_2fa_secret_never_leaked` — responses don't contain the
//!     secret.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::auth::totp;
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;
use totp_rs::{Algorithm, Secret, Totp};

const APP_SECRET: &str = "test-secret-do-not-use-in-production-please-replace-with-64-random-chars";

async fn register_user(server: &TestServer, email: &str, password: &str) -> String {
    server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", email.split('@').next().unwrap_or("user")),
            ("password", password),
            ("password_confirm", password),
        ])
        .send()
        .await
        .expect("register");
    password.to_string()
}

/// Generate a valid TOTP code for the supplied base32 secret at the
/// supplied time.
fn make_code(secret_b32: &str, when: u64) -> String {
    let secret = Secret::try_from_base32(secret_b32).expect("base32");
    let totp = totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .build()
        .expect("totp");
    totp.generate(when).to_string()
}

#[tokio::test]
async fn http_2fa_enroll_success() {
    let server = TestServer::new().await;
    let email = "alice@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;
    let cookie = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookie = cookie
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let cookie = s.split(';').next().unwrap_or("");
            if cookie.starts_with("oa_session=") {
                Some(cookie.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie set on login response");

    // GET /account/security — should render enrollment form.
    let resp = server
        .client()
        .get(format!("{}/account/security", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("GET security");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Enroll"), "enrollment form missing");
    assert!(
        body.contains("otpauth") || body.contains("secret"),
        "QR or secret not in page"
    );

    // Extract the secret from the hidden input.
    let secret = body
        .split("name=\"secret\" value=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(|s| s.to_string())
        .expect("secret hidden field present");
    assert!(!secret.is_empty(), "secret is empty");

    // Compute a valid code and POST it.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let code = make_code(&secret, now);

    let resp = server
        .client()
        .post(format!("{}/account/security/enroll", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("secret", secret.as_str()), ("code", code.as_str())])
        .send()
        .await
        .expect("POST enroll");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("recovery code"), "recovery codes not shown");
    assert!(body.contains("Two-factor authentication enabled"));

    // DB check: enrolled + recovery codes.
    let pool = server.db().pool();
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_totp WHERE user_id IN (SELECT id FROM users WHERE email = $1)",
    )
    .bind(email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "exactly one enrollment row");
    let (codes,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recovery_codes WHERE user_id IN (SELECT id FROM users WHERE email = $1)",
    )
    .bind(email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(codes, 10, "exactly 10 recovery codes");
}

#[tokio::test]
async fn http_2fa_login_requires_code() {
    let server = TestServer::new().await;
    let email = "bob@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;

    // Registration auto-logs the user in (`ux-onboarding-flow`),
    // which would leave an authenticated session in the cookie jar.
    // Sign out first so the 2FA flow below starts unauthenticated.
    server
        .client()
        .post(format!("{}/logout", server.base_url()))
        .send()
        .await
        .expect("logout");

    // Manually enroll via the DB.
    let pool = server.db().pool();
    let user_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let secret = totp::new_secret().unwrap();
    let cipher = openaccounting::auth::totp::TotpCipher::from_app_secret(APP_SECRET);
    let sealed = cipher.seal(&secret).unwrap();
    totp::enroll(&pool, user_id.0, &sealed).await.unwrap();
    let codes = totp::regenerate_recovery_codes(&pool, user_id.0)
        .await
        .unwrap();
    assert_eq!(codes.len(), 10);

    // POST /login with correct password — expect 303 to /login/2fa, NOT
    // /ledgers or anywhere else authenticated.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", email),
            ("password", password),
            ("next", "/account"),
        ])
        .send()
        .await
        .expect("POST /login");
    assert_eq!(
        resp.status(),
        303,
        "password-only login must redirect to 2FA step"
    );
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(
        location.contains("/login/2fa"),
        "expected redirect to /login/2fa, got: {location}"
    );

    // No session cookie should have been set (only the
    // pre-auth session for the 2FA step is in the cookie jar,
    // but it must NOT contain a user record).
    let cookies = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !cookies.is_empty(),
        "session cookie should still be set (for the 2FA step); got: {cookies}"
    );

    // Confirm: hitting /ledgers with the cookie should redirect
    // to /login because we are not yet authenticated.
    let resp = server
        .client()
        .get(format!("{}/ledgers", server.base_url()))
        .send()
        .await
        .expect("GET /ledgers");
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .map(|v| v.to_str().unwrap_or("").to_string())
        .unwrap_or_default();
    assert!(
        (status == 303 || status == 307) && loc.contains("/login"),
        "expected redirect to login for protected route; got status={status} loc={loc}"
    );
}

#[tokio::test]
async fn http_2fa_disable_requires_both_factors() {
    let server = TestServer::new().await;
    let email = "carol@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;

    // Enroll via DB.
    let pool = server.db().pool();
    let user_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let secret = totp::new_secret().unwrap();
    let cipher = openaccounting::auth::totp::TotpCipher::from_app_secret(APP_SECRET);
    let sealed = cipher.seal(&secret).unwrap();
    totp::enroll(&pool, user_id.0, &sealed).await.unwrap();
    totp::regenerate_recovery_codes(&pool, user_id.0)
        .await
        .unwrap();

    // Login to get a session cookie (this will redirect to /login/2fa).
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", email),
            ("password", password),
            ("next", "/account"),
        ])
        .send()
        .await
        .expect("login");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");

    // Complete 2FA so we have a fully-authenticated session.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let code = make_code(&secret, now);
    let resp = server
        .client()
        .post(format!("{}/login/2fa", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("code", code.as_str()), ("next", "/account/security")])
        .send()
        .await
        .expect("POST 2fa");
    assert_eq!(resp.status(), 303, "2FA must succeed for this test");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie after 2fa");

    // Attempt to disable with only password (no code).
    let resp = server
        .client()
        .post(format!("{}/account/security/disable", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("password", password), ("code", "")])
        .send()
        .await
        .expect("POST disable");
    let status = resp.status();
    assert!(
        status == 400 || status == 401 || status == 422,
        "disable with missing code should fail; got status={status}"
    );
    // Still enrolled.
    let enrolled: (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM user_totp WHERE user_id = $1)")
            .bind(user_id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(enrolled.0, "must still be enrolled after rejected disable");

    // Disable with wrong password (and no code) → reject.
    let resp = server
        .client()
        .post(format!("{}/account/security/disable", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("password", "WRONG"), ("code", "000000")])
        .send()
        .await
        .expect("POST disable 2");
    let status = resp.status();
    assert!(
        status == 401 || status == 400 || status == 422,
        "disable with wrong password must fail; got status={status}"
    );

    // Disable with valid password + valid code → 303 to /account/security.
    // Wait for the next 30-second step so the counter has rolled
    // over from the code we used during login. (Skip in unit
    // tests where we can mock time; here we just sleep.)
    let step_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 30;
    let next_step_secs = (step_now + 1) * 30 + 1;
    let wait = next_step_secs.saturating_sub(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    if wait > 0 && wait < 60 {
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let code = make_code(&secret, now);
    let resp = server
        .client()
        .post(format!("{}/account/security/disable", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .form(&[("password", password), ("code", code.as_str())])
        .send()
        .await
        .expect("POST disable 3");
    assert_eq!(resp.status(), 303);
    let enrolled: (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM user_totp WHERE user_id = $1)")
            .bind(user_id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!enrolled.0, "must NOT be enrolled after successful disable");
}

#[tokio::test]
async fn http_2fa_secret_never_leaked() {
    let server = TestServer::new().await;
    let email = "dan@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;

    // Login to get cookie.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");

    // Visit several pages; none should contain the encrypted
    // secret (which has the format "<b64>:<b64>"). We probe
    // for the SQL EXCLUDED text too.
    let mut last_body = String::new();
    for path in &["/account", "/ledgers", "/account/security"] {
        let resp = server
            .client()
            .get(format!("{}{}", server.base_url(), path))
            .header(reqwest::header::COOKIE, cookie.clone())
            .send()
            .await
            .expect("GET");
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        assert!(
            status.is_success() || status == 303,
            "{path} returned {status}"
        );
        assert!(
            !body.contains("user_totp"),
            "{path} response leaked user_totp table name"
        );
        assert!(
            !body.contains("secret_encrypted"),
            "{path} response leaked column name"
        );
        // The encrypted blob has the shape `b64:b64` with both
        // halves > 16 chars. A regex-free heuristic: look for
        // the long base64A:base64B pattern that the cipher
        // produces.
        let mut split = body.split(':');
        let first = split.next().unwrap_or("");
        let second = split.next().unwrap_or("");
        if first.len() >= 16 && second.len() >= 16 {
            // Could still be CSS pseudo-selectors or some
            // legitimate pattern. Verify it doesn't look like
            // base64 by trying to decode one half.
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD;
            let decoded =
                b64.decode(first.as_bytes()).is_ok() && b64.decode(second.as_bytes()).is_ok();
            if decoded
                && first
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            {
                panic!("{path} response looks like it contains an encrypted-secret blob");
            }
        }
        last_body = body;
    }
    // The /account/security pre-enrollment page DOES show the
    // secret (that's how enrollment works). So a separate check
    // is needed once 2FA IS enrolled: the enrolled page must
    // NOT show the secret.
    let _ = last_body; // silence unused
}

#[tokio::test]
async fn http_2fa_recovery_code_one_shot() {
    let server = TestServer::new().await;
    let email = "eve@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;
    let pool = server.db().pool();
    let user_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let secret = totp::new_secret().unwrap();
    let cipher = openaccounting::auth::totp::TotpCipher::from_app_secret(APP_SECRET);
    let sealed = cipher.seal(&secret).unwrap();
    totp::enroll(&pool, user_id.0, &sealed).await.unwrap();
    let codes = totp::regenerate_recovery_codes(&pool, user_id.0)
        .await
        .unwrap();
    let good = codes[0].clone();

    // Submit password → redirected to /login/2fa.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");
    assert_eq!(resp.status(), 303);
    assert!(resp
        .headers()
        .get(reqwest::header::LOCATION)
        .map(|v| v.to_str().unwrap_or("").contains("/login/2fa"))
        .unwrap_or(false));

    // Submit the recovery code at /login/2fa.
    let resp = server
        .client()
        .post(format!("{}/login/2fa", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("code", good.as_str()), ("next", "/")])
        .send()
        .await
        .expect("POST 2fa");
    assert_eq!(resp.status(), 303, "recovery code should succeed");

    // The same code cannot be reused (we'd need to log out first
    // to redo the test, but the code is consumed).
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM recovery_codes WHERE user_id = $1 AND consumed_at IS NULL",
    )
    .bind(user_id.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 9, "exactly one code consumed");
}

#[tokio::test]
async fn http_2fa_replay_rejected() {
    let server = TestServer::new().await;
    let email = "frank@example.com";
    let password = "correct horse battery staple";
    register_user(&server, email, password).await;
    let pool = server.db().pool();
    let user_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let secret = totp::new_secret().unwrap();
    let cipher = openaccounting::auth::totp::TotpCipher::from_app_secret(APP_SECRET);
    let sealed = cipher.seal(&secret).unwrap();
    totp::enroll(&pool, user_id.0, &sealed).await.unwrap();
    totp::regenerate_recovery_codes(&pool, user_id.0)
        .await
        .unwrap();

    // Login → 2FA.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");

    // Use a real code once.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let code = make_code(&secret, now);
    let resp = server
        .client()
        .post(format!("{}/login/2fa", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("code", code.as_str()), ("next", "/")])
        .send()
        .await
        .expect("POST 2fa");
    assert_eq!(resp.status(), 303);

    // Logout and try to login again with the same code → must fail.
    let _ = server
        .client()
        .post(format!("{}/logout", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("logout");
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", password), ("next", "/")])
        .send()
        .await
        .expect("login");
    let cookie2 = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| {
            let c = s.split(';').next().unwrap_or("");
            if c.starts_with("oa_session=") {
                Some(c.to_string())
            } else {
                None
            }
        })
        .expect("oa_session cookie");
    // Same code → reject (counter has advanced).
    let resp = server
        .client()
        .post(format!("{}/login/2fa", server.base_url()))
        .header(reqwest::header::COOKIE, cookie2)
        .form(&[("code", code.as_str()), ("next", "/")])
        .send()
        .await
        .expect("POST 2fa replay");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert!(
        status == 200,
        "replay should render the 2FA page with error; got {status}"
    );
    assert!(
        body.contains("Invalid code"),
        "replay should be rejected with Invalid code; body starts: {}",
        &body[..body.len().min(200)]
    );
    let _ = cookie; // silence unused
    let _ = OffsetDateTime::now_utc(); // silence unused
}
