//! HTTP integration tests for the password-strength policy
//! (`s4-password-strength`).
//!
//! - Minimum length is 12 (was 8).
//! - Common passwords are rejected (bundled deny list).
//! - Change password enforces the same policy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;

const STRONG: &str = "X7!qZ4wN9pLk_3vR"; // 16 chars, mixed case + digits + symbols

#[tokio::test]
async fn http_register_rejects_short_password() {
    let server = TestServer::new().await;

    // 11 chars — under the new minimum of 12.
    let short = "Short1!aaaa"; // exactly 12 — should pass length, but...
                               // Actually need an 11-char one.
    let eleven = "Short1!aaa"; // 11
    let resp = server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", "alice@example.com"),
            ("username", "alice"),
            ("password", eleven),
            ("password_confirm", eleven),
        ])
        .send()
        .await
        .expect("POST /register");
    assert_eq!(
        resp.status(),
        200,
        "register must render the page on validation failure"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("at least 12"),
        "error message must mention the new minimum; got: {}",
        &body[..body.len().min(400)]
    );

    // Also: 12 chars but exactly = MIN_LENGTH — must succeed.
    let twelve = "abcdefghijkl"; // 12 lowercase, not common
    let resp = server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", "bob@example.com"),
            ("username", "bob"),
            ("password", twelve),
            ("password_confirm", twelve),
        ])
        .send()
        .await
        .expect("POST /register");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "12-char non-common password must be accepted; got {}",
        resp.status()
    );
    let _ = short;
    let _ = STRONG;
}

#[tokio::test]
async fn http_register_rejects_common_password() {
    let server = TestServer::new().await;

    // "Password123" is on the deny list (length 11 — both checks fail).
    let bad = "Password123";
    let resp = server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", "alice@example.com"),
            ("username", "alice"),
            ("password", bad),
            ("password_confirm", bad),
        ])
        .send()
        .await
        .expect("POST /register");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("too common") || body.contains("at least 12"),
        "common password must be rejected; got: {}",
        &body[..body.len().min(400)]
    );

    // A different common one — "qwerty" (6 chars, too short, but
    // also common).
    let bad2 = "qwerty";
    let resp = server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", "bob@example.com"),
            ("username", "bob"),
            ("password", bad2),
            ("password_confirm", bad2),
        ])
        .send()
        .await
        .expect("POST /register");
    let body = resp.text().await.unwrap();
    assert!(body.contains("at least 12") || body.contains("too common"));
}

#[tokio::test]
async fn http_change_password_enforces_policy() {
    let server = TestServer::new().await;
    let pool = server.db().pool();

    let email = "carol@example.com";
    // Use a strong password for the seed.
    let resp = server
        .client()
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email),
            ("username", "carol"),
            ("password", STRONG),
            ("password_confirm", STRONG),
        ])
        .send()
        .await
        .expect("POST /register");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "seed register must succeed; got {}",
        resp.status()
    );

    // Log in to capture the session cookie.
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", STRONG), ("next", "/")])
        .send()
        .await
        .expect("POST /login");
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

    // Attempt to change to a too-short password — must fail.
    let too_short = "Short1!";
    let resp = server
        .client()
        .post(format!("{}/account/password", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("current_password", STRONG),
            ("new_password", too_short),
            ("confirm_password", too_short),
        ])
        .send()
        .await
        .expect("POST /account/password");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert!(
        status == 200 || status == 400 || status == 422,
        "too-short password change should return page or 4xx; got {status}"
    );
    assert!(
        body.contains("at least 12"),
        "response must mention new minimum; got: {}",
        &body[..body.len().min(400)]
    );

    // Attempt to change to a common password — must fail.
    let common = "Password1234"; // 12 chars but in deny list
    let resp = server
        .client()
        .post(format!("{}/account/password", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("current_password", STRONG),
            ("new_password", common),
            ("confirm_password", common),
        ])
        .send()
        .await
        .expect("POST /account/password");
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert!(
        status == 200 || status == 400 || status == 422,
        "common-password change should return page or 4xx; got {status}"
    );
    assert!(
        body.contains("too common"),
        "response must explain why; got: {}",
        &body[..body.len().min(400)]
    );

    // Strong new password — must succeed.
    let new_strong = "n3wStr0ng-Pa55!";
    let resp = server
        .client()
        .post(format!("{}/account/password", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[
            ("current_password", STRONG),
            ("new_password", new_strong),
            ("confirm_password", new_strong),
        ])
        .send()
        .await
        .expect("POST /account/password");
    assert!(
        resp.status().is_success() || resp.status().as_u16() == 303,
        "strong new password must be accepted; got {}",
        resp.status()
    );

    // Verify the DB reflects the change.
    let row: (String,) = sqlx::query_as("SELECT hashed_password FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        openaccounting::auth::password::verify_password(new_strong, &row.0).unwrap(),
        "new password must verify after change"
    );
}
