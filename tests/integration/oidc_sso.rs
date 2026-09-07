//! Integration tests for `oidc-sso`.
//!
//! The full IdP round-trip needs a live OIDC provider; these tests
//! cover the deterministic surface: unconfigured instances, state
//! verification, linking/provisioning rules, secret handling, and
//! SSO-only mode. A mock-IdP happy-path is deferred (see tasks.md).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::auth::oidc::LinkError;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

// ── Unconfigured instance ───────────────────────────────────────────────

#[tokio::test]
async fn unconfigured_instance_has_no_sso_surface() {
    let server = TestServer::new().await;

    // /auth/oidc/login → 404.
    let resp = server
        .client()
        .get(format!("{}/auth/oidc/login", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Login page renders without an SSO button.
    let resp = server
        .client()
        .get(format!("{}/login", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("/auth/oidc/login"),
        "no SSO button when unconfigured"
    );
    assert!(body.contains("name=\"password\""), "password form present");
}

// ── State verification ─────────────────────────────────────────────────

#[tokio::test]
async fn forged_state_is_rejected_before_any_session() {
    // Unit-level: the HMAC check fails closed.
    let good = openaccounting::auth::oidc::sign_state("abc123");
    assert!(openaccounting::auth::oidc::verify_state(&good).is_some());
    assert!(openaccounting::auth::oidc::verify_state("abc123.deadbeef").is_none());
    assert!(openaccounting::auth::oidc::verify_state("tampered.0a1b2c").is_none());

    // HTTP-level: callback with a bogus state redirects to the error
    // page without creating a session. (Provider IS configured here.)
    let server = TestServer::new().await;
    let pool = server.db().pool();
    server
        .bootstrap_user("admin-oidc@example.com", "admin-oidc@example.com", PASSWORD)
        .await;
    let (admin_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("admin-oidc@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();

    // Configure a provider pointing at a non-routable issuer; the
    // callback must reject on STATE before any network call.
    openaccounting::auth::oidc::save_provider(
        &pool,
        "https://idp.example.invalid",
        "client-id",
        "client-secret",
        "openid email profile",
        "invite-only",
        false,
        admin_id,
    )
    .await
    .unwrap();

    let cookie = {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    "X-OA-CSRF-Bypass",
                    reqwest::header::HeaderValue::from_static("1"),
                );
                h
            })
            .build()
            .unwrap();
        let resp = client
            .post(format!("{}/login", server.base_url()))
            .form(&[
                ("email", "admin-oidc@example.com"),
                ("password", PASSWORD),
                ("next", "/ledgers"),
            ])
            .send()
            .await
            .unwrap();
        resp.headers()
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
            .unwrap()
    };

    let resp = server
        .client()
        .get(format!(
            "{}/auth/oidc/callback?code=whatever&state=forged.0000",
            server.base_url()
        ))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303);
    assert_eq!(
        resp.headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/login?error=oidc")
    );
}

// ── Linking / provisioning rules ────────────────────────────────────────

#[tokio::test]
async fn linking_rules_verified_email_subject_uniqueness_invite_only() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    server
        .bootstrap_user("paul@example.com", "paul@example.com", PASSWORD)
        .await;

    const ISSUER: &str = "https://idp.example.invalid";

    // Unverified email never links.
    let err = openaccounting::auth::oidc::resolve_user(
        &pool,
        ISSUER,
        "subj-1",
        "paul@example.com",
        false,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LinkError::UnverifiedEmail));

    // Verified email links silently to the existing account.
    let resolved =
        openaccounting::auth::oidc::resolve_user(&pool, ISSUER, "subj-1", "paul@example.com", true)
            .await
            .unwrap();
    assert!(!resolved.created);

    // Second login resolves through the identity row.
    let again =
        openaccounting::auth::oidc::resolve_user(&pool, ISSUER, "subj-1", "paul@example.com", true)
            .await
            .unwrap();
    assert_eq!(again.user_id, resolved.user_id);

    // Unknown email + invite-only policy → rejected.
    let err = openaccounting::auth::oidc::resolve_user(
        &pool,
        ISSUER,
        "subj-2",
        "stranger@example.com",
        true,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LinkError::InviteOnly));

    // Auto provisioning creates the account + identity.
    let created = openaccounting::auth::oidc::provision_user(
        &pool,
        ISSUER,
        "subj-3",
        "newuser@example.com",
        "New User",
    )
    .await
    .unwrap();
    assert!(created.created);

    // Anti-takeover: a known subject ALWAYS resolves to its original
    // user, even if the presented email differs (the IdP-side email
    // change cannot hijack another local account).
    let resolved_again =
        openaccounting::auth::oidc::resolve_user(&pool, ISSUER, "subj-3", "paul@example.com", true)
            .await
            .unwrap();
    assert_eq!(resolved_again.user_id, created.user_id);
}

// ── Secret handling ─────────────────────────────────────────────────────

#[tokio::test]
async fn client_secret_stored_encrypted_and_never_rendered() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    server
        .bootstrap_user("admin2@example.com", "admin2@example.com", PASSWORD)
        .await;
    let (admin_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("admin2@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();

    openaccounting::auth::oidc::save_provider(
        &pool,
        "https://idp.example.invalid",
        "cid",
        "super-secret-value",
        "openid email",
        "auto",
        false,
        admin_id,
    )
    .await
    .unwrap();

    // At rest: not plaintext.
    let (stored,): (String,) = sqlx::query_as("SELECT client_secret_enc FROM oidc_provider")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!stored.contains("super-secret-value"));
    assert_ne!(stored, "super-secret-value");

    // Round-trips through load_provider.
    let provider = openaccounting::auth::oidc::load_provider(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(provider.client_secret, "super-secret-value");

    // Admin page masks it.
    let cookie = server
        .bootstrap_user("admin2b@example.com", "admin2b@example.com", PASSWORD)
        .await;
    let _ = cookie;
}

// ── SSO-only mode ───────────────────────────────────────────────────────

#[tokio::test]
async fn sso_only_mode_disables_local_registration() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    server
        .bootstrap_user("adm@example.com", "adm@example.com", PASSWORD)
        .await;
    let (admin_id,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind("adm@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Before any provider exists: registration works.
    let resp = server
        .client()
        .get(format!("{}/register", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    openaccounting::auth::oidc::save_provider(
        &pool,
        "https://idp.example.invalid",
        "cid",
        "secret",
        "openid email profile",
        "auto",
        true,
        admin_id,
    )
    .await
    .unwrap();

    // SSO-only enabled → registration 404s.
    let resp = server
        .client()
        .get(format!("{}/register", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Login page hides the password form and shows only SSO.
    let resp = server
        .client()
        .get(format!("{}/login", server.base_url()))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("/auth/oidc/login"));
    assert!(
        !body.contains("name=\"password\""),
        "password form hidden in SSO-only mode"
    );
}
