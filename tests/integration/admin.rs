//! HTTP integration tests for the admin console (`a11-admin-console`).
//!
//! Covers:
//! - Non-admin users get 403 on `/admin/*`.
//! - Suspend / activate a user via `POST /admin/users/{id}/status`;
//!   a suspended user is refused at login.
//! - Promote / demote a user via `POST /admin/users/{id}/role`.
//! - Self-suspend and last-active-admin demotion are refused (422).
//! - Every management action is audit-logged.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use uuid::Uuid;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
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
        .unwrap()
}

/// Register + log in a user with a dedicated cookie jar, returning
/// the client (signed in) and the user id.
async fn register_user(server: &TestServer, tag: &str) -> (reqwest::Client, Uuid) {
    let client = make_client();
    let email = format!("{tag}@example.com");
    client
        .post(format!("{}/register", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("username", tag),
            ("password", PASSWORD),
            ("password_confirm", PASSWORD),
        ])
        .send()
        .await
        .expect("register");
    client
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", email.as_str()),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .expect("login");
    let pool = server.db().pool();
    let id: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .expect("user id");
    (client, id.0)
}

/// Promote `email` to admin in the DB, then re-login so the session
/// picks up the admin role; returns the signed-in client.
async fn promote_and_relogin(server: &TestServer, client: &reqwest::Client, email: &str) {
    sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1")
        .bind(email)
        .execute(&server.db().pool())
        .await
        .expect("promote");
    client
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD), ("next", "/admin")])
        .send()
        .await
        .expect("re-login after promote");
}

async fn user_id_of(server: &TestServer, email: &str) -> Uuid {
    let pool = server.db().pool();
    let id: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(&pool)
        .await
        .expect("user id");
    id.0
}

#[tokio::test]
async fn admin_non_admin_blocked_403() {
    let server = TestServer::new().await;
    let (client, _) = register_user(&server, "plain-user-403").await;
    let resp = client
        .get(format!("{}/admin/users", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "non-admin must get 403 on /admin/users");
}

#[tokio::test]
async fn admin_suspend_user_blocks_login() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "susp-admin").await;
    promote_and_relogin(&server, &admin, "susp-admin@example.com").await;

    let (target, target_id) = register_user(&server, "susp-target").await;
    let _ = target;

    // Suspend the target.
    let resp = admin
        .post(format!(
            "{}/admin/users/{target_id}/status",
            server.base_url()
        ))
        .form(&[("is_active", "false")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 303, "suspend should redirect");

    // The suspended user's next login is refused with the generic error.
    let login = make_client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", "susp-target@example.com"),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .unwrap();
    let body = login.text().await.unwrap();
    assert!(
        body.contains("Invalid email or password"),
        "suspended user must be refused at login; body: {body}"
    );
}

#[tokio::test]
async fn admin_activate_user_restores_login() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "act-admin").await;
    promote_and_relogin(&server, &admin, "act-admin@example.com").await;

    // Create the target user, then suspend + reactivate it.
    register_user(&server, "act-target").await;
    let target_id = user_id_of(&server, "act-target@example.com").await;
    admin
        .post(format!(
            "{}/admin/users/{target_id}/status",
            server.base_url()
        ))
        .form(&[("is_active", "false")])
        .send()
        .await
        .unwrap();
    // Reactivate.
    admin
        .post(format!(
            "{}/admin/users/{target_id}/status",
            server.base_url()
        ))
        .form(&[("is_active", "true")])
        .send()
        .await
        .unwrap();

    let login = make_client()
        .post(format!("{}/login", server.base_url()))
        .form(&[
            ("email", "act-target@example.com"),
            ("password", PASSWORD),
            ("next", "/ledgers"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        login.status(),
        303,
        "reactivated user must be able to log in"
    );
}

#[tokio::test]
async fn admin_suspend_is_audited() {
    let server = TestServer::new().await;

    let (admin, admin_id) = register_user(&server, "audit-admin").await;
    promote_and_relogin(&server, &admin, "audit-admin@example.com").await;

    register_user(&server, "audit-target").await;
    let target_id = user_id_of(&server, "audit-target@example.com").await;

    admin
        .post(format!(
            "{}/admin/users/{target_id}/status",
            server.base_url()
        ))
        .form(&[("is_active", "false")])
        .send()
        .await
        .unwrap();

    let row: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT actor_id, new_value FROM audit_entries
         WHERE entity_type = 'user' AND entity_id = $1 AND action = 'set_status'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(target_id)
    .fetch_optional(&server.db().pool())
    .await
    .unwrap();
    let (actor_id, new_value) = row.expect("audit row for suspend");
    assert_eq!(actor_id, admin_id, "actor must be the acting admin");
    assert_eq!(
        new_value.get("is_active").and_then(|v| v.as_bool()),
        Some(false),
        "new_value.is_active must be false"
    );
}

#[tokio::test]
async fn admin_promote_user() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "promo-admin").await;
    promote_and_relogin(&server, &admin, "promo-admin@example.com").await;

    register_user(&server, "promo-target").await;
    let target_id = user_id_of(&server, "promo-target@example.com").await;

    admin
        .post(format!(
            "{}/admin/users/{target_id}/role",
            server.base_url()
        ))
        .form(&[("role", "admin")])
        .send()
        .await
        .unwrap();

    let role: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(target_id)
        .fetch_one(&server.db().pool())
        .await
        .unwrap();
    assert_eq!(role.0, "admin", "target must be promoted to admin");

    let audit: Option<(Uuid,)> = sqlx::query_as(
        "SELECT entity_id FROM audit_entries
         WHERE entity_type = 'user' AND entity_id = $1 AND action = 'set_role'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(target_id)
    .fetch_optional(&server.db().pool())
    .await
    .unwrap();
    assert!(audit.is_some(), "promotion must be audit-logged");
}

#[tokio::test]
async fn admin_demote_admin() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "demo-admin-a").await;
    promote_and_relogin(&server, &admin, "demo-admin-a@example.com").await;

    // Second admin to demote.
    let (other, other_id) = register_user(&server, "demo-admin-b").await;
    let _ = other;
    promote_and_relogin(&server, &admin, "demo-admin-a@example.com").await;
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(other_id)
        .execute(&server.db().pool())
        .await
        .unwrap();

    admin
        .post(format!("{}/admin/users/{other_id}/role", server.base_url()))
        .form(&[("role", "user")])
        .send()
        .await
        .unwrap();

    let role: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(other_id)
        .fetch_one(&server.db().pool())
        .await
        .unwrap();
    assert_eq!(role.0, "user", "target must be demoted to user");
}

#[tokio::test]
async fn admin_self_suspend_422() {
    let server = TestServer::new().await;

    let (admin, admin_id) = register_user(&server, "self-susp-admin").await;
    promote_and_relogin(&server, &admin, "self-susp-admin@example.com").await;

    let resp = admin
        .post(format!(
            "{}/admin/users/{admin_id}/status",
            server.base_url()
        ))
        .form(&[("is_active", "false")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422, "self-suspend must be refused");

    let active: (bool,) = sqlx::query_as("SELECT is_active FROM users WHERE id = $1")
        .bind(admin_id)
        .fetch_one(&server.db().pool())
        .await
        .unwrap();
    assert!(active.0, "admin must still be active");
}

#[tokio::test]
async fn admin_last_admin_demote_422() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "last-admin-a").await;
    promote_and_relogin(&server, &admin, "last-admin-a@example.com").await;

    // Second admin, then suspend them directly so only admin A is
    // an active admin.
    let (other, other_id) = register_user(&server, "last-admin-b").await;
    let _ = other;
    promote_and_relogin(&server, &admin, "last-admin-a@example.com").await;
    sqlx::query("UPDATE users SET role = 'admin', is_active = FALSE WHERE id = $1")
        .bind(other_id)
        .execute(&server.db().pool())
        .await
        .unwrap();

    let resp = admin
        .post(format!("{}/admin/users/{other_id}/role", server.base_url()))
        .form(&[("role", "user")])
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        422,
        "demoting an admin while only one active admin remains must be refused"
    );
}

#[tokio::test]
async fn admin_user_detail_shows_ledgers_and_activity() {
    let server = TestServer::new().await;

    let (admin, _) = register_user(&server, "detail-admin").await;
    promote_and_relogin(&server, &admin, "detail-admin@example.com").await;

    // Target user with a ledger (its creation is audit-logged).
    let (target, target_id) = register_user(&server, "detail-target").await;
    let resp = target
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "detail-books"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection(), "ledger create redirect");

    // Find the ledger id from the Location header.
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let ledger_id: Uuid = loc.rsplit('/').next().unwrap().parse().unwrap();
    let _ = ledger_id;

    // The audit log records the ledger creation (actor = target).
    let resp = admin
        .get(format!("{}/admin/users/{target_id}", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "user detail must render");
    let body = resp.text().await.unwrap();
    assert!(body.contains("detail-target"), "username shown");
    assert!(body.contains("detail-books"), "owned ledger shown");
    assert!(
        body.contains("Recent activity") && body.contains("ledger"),
        "activity section rendered"
    );
}
