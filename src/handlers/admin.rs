use axum::{
    extract::{Form, Path, State},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    Router,
};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    templates::{
        admin::{AdminDashboardPage, AdminUsersPage, RecentUserRow, UserRow},
        render_response,
    },
    AppState,
};

/// Middleware: require the authenticated user to have `role = 'admin'`.
pub async fn require_admin(
    auth: AuthSession<Backend>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}

/// Admin-only router, layered with `require_admin`.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin", axum::routing::get(dashboard))
        .route("/admin/users", axum::routing::get(users))
        .route(
            "/admin/users/{id}/status",
            axum::routing::post(set_user_status),
        )
        .route("/admin/users/{id}/role", axum::routing::post(set_user_role))
        .route(
            "/admin/ocr-corpus.json",
            axum::routing::get(crate::handlers::document_ocr_feedback::export_corpus),
        )
        .route(
            "/admin/audit/verify",
            axum::routing::get(crate::handlers::admin_audit_verify::verify_chain),
        )
        .route_layer(middleware::from_fn(require_admin))
}

pub async fn dashboard(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<impl axum::response::IntoResponse> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let total_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;

    let total_ledgers: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ledgers")
        .fetch_one(&state.pool)
        .await?;

    let total_transactions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
        .fetch_one(&state.pool)
        .await?;

    #[derive(sqlx::FromRow)]
    struct RecentUser {
        username: String,
        email: String,
        role: String,
        created_at: time::OffsetDateTime,
    }

    let recent_users = sqlx::query_as::<_, RecentUser>(
        "SELECT username, email, role, created_at FROM users ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await?;

    let page = AdminDashboardPage::new(
        user.clone(),
        total_users.0,
        total_ledgers.0,
        total_transactions.0,
        recent_users
            .into_iter()
            .map(|u| RecentUserRow {
                username: u.username,
                email: u.email,
                role: u.role,
                created_at_display: crate::templates::account::fmt_date(&u.created_at),
            })
            .collect(),
    );

    Ok(render_response(page))
}

pub async fn users(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<impl axum::response::IntoResponse> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    #[derive(sqlx::FromRow)]
    struct UserRowDb {
        id: Uuid,
        username: String,
        email: String,
        role: String,
        is_active: bool,
        created_at: time::OffsetDateTime,
    }

    let rows = sqlx::query_as::<_, UserRowDb>(
        "SELECT id, username, email, role, is_active, created_at FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let page = AdminUsersPage::new(
        user.clone(),
        rows.into_iter()
            .map(|r| UserRow {
                id: r.id,
                username: r.username,
                email: r.email,
                role: r.role,
                is_active: r.is_active,
                is_self: r.id == user.id,
                created_at_display: crate::templates::account::fmt_date(&r.created_at),
            })
            .collect(),
    );

    Ok(render_response(page))
}

/// Form body for `POST /admin/users/{id}/status` — the target's
/// desired `is_active` value.
#[derive(Deserialize)]
pub struct UserStatusForm {
    pub is_active: bool,
}

/// Suspend or activate a user (`a11-admin-console`). Suspension is
/// enforced at login (`src/auth/mod.rs`); the toggle is audit-logged.
pub async fn set_user_status(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Form(form): Form<UserStatusForm>,
) -> AppResult<Redirect> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    // Guard rail: an admin must not be able to suspend their own
    // account (`a11-admin-console` Self-Protection).
    if user_id == admin.id && !form.is_active {
        return Err(AppError::Unprocessable(
            "You cannot suspend your own account.".into(),
        ));
    }

    let current: Option<(String, bool)> =
        sqlx::query_as("SELECT email, is_active FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((email, was_active)) = current else {
        return Err(AppError::NotFound);
    };
    let _ = email;
    if was_active == form.is_active {
        // No-op: redirect without writing an audit row.
        return Ok(Redirect::to("/admin/users"));
    }

    sqlx::query("UPDATE users SET is_active = $1 WHERE id = $2")
        .bind(form.is_active)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    audit::log(
        &state.pool,
        None,
        admin.id,
        "set_status",
        "user",
        Some(user_id),
        Some(serde_json::json!({ "is_active": was_active })),
        Some(serde_json::json!({ "is_active": form.is_active })),
    )
    .await?;

    Ok(Redirect::to("/admin/users"))
}

/// Form body for `POST /admin/users/{id}/role` — the target's
/// desired role (`admin` or `user`).
#[derive(Deserialize)]
pub struct UserRoleForm {
    pub role: String,
}

/// Promote or demote a user (`a11-admin-console`). The audit log
/// records the change.
pub async fn set_user_role(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Form(form): Form<UserRoleForm>,
) -> AppResult<Redirect> {
    let admin = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    if form.role != "admin" && form.role != "user" {
        return Err(AppError::Unprocessable(format!(
            "Invalid role: {}",
            form.role
        )));
    }
    // Guard rail: an admin must not change their own role
    // (`a11-admin-console` Self-Protection).
    if user_id == admin.id {
        return Err(AppError::Unprocessable(
            "You cannot change your own role.".into(),
        ));
    }

    let current: Option<(String,)> = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((old_role,)) = current else {
        return Err(AppError::NotFound);
    };
    if old_role == form.role {
        return Ok(Redirect::to("/admin/users"));
    }

    // Guard rail: never leave the system without an active admin
    // (`a11-admin-console` Self-Protection). Refuse any admin
    // demotion while exactly one active admin remains (the acting
    // admin themselves), since it could drop the count to zero.
    if form.role != "admin" && old_role == "admin" {
        let active_admins: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE")
                .fetch_one(&state.pool)
                .await?;
        if active_admins.0 <= 1 {
            return Err(AppError::Unprocessable(
                "Cannot demote the last active admin.".into(),
            ));
        }
    }

    sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(&form.role)
        .bind(user_id)
        .execute(&state.pool)
        .await?;

    audit::log(
        &state.pool,
        None,
        admin.id,
        "set_role",
        "user",
        Some(user_id),
        Some(serde_json::json!({ "role": old_role })),
        Some(serde_json::json!({ "role": form.role })),
    )
    .await?;

    Ok(Redirect::to("/admin/users"))
}

#[cfg(test)]
mod tests {
    use crate::auth::{password::hash_password, User};

    async fn setup() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .ok()?;
        Some(pool)
    }

    async fn create_test_user(pool: &sqlx::PgPool, tag: &str, role: &str) -> User {
        let email = format!("{tag}@example.com");
        let username = tag.to_string();
        let hashed = hash_password("password123").unwrap();
        sqlx::query_as::<_, User>(
            r#"INSERT INTO users (email, username, hashed_password, display_name, role)
               VALUES ($1, $2, $3, $1, $4)
               ON CONFLICT (email) DO UPDATE SET role = $4
               RETURNING id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale"#,
        )
        .bind(&email)
        .bind(&username)
        .bind(&hashed)
        .bind(role)
        .fetch_one(pool)
        .await
        .expect("create test user")
    }

    async fn cleanup(pool: &sqlx::PgPool, email: &str) {
        let _ = sqlx::query("DELETE FROM users WHERE email = $1")
            .bind(email)
            .execute(pool)
            .await;
    }

    #[tokio::test]
    async fn admin_role_is_stored_and_retrieved() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "admin-role-check", "admin").await;
        assert_eq!(user.role, "admin");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn user_role_is_stored_and_retrieved() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "user-role-check", "user").await;
        assert_ne!(user.role, "admin");
        assert_eq!(user.role, "user");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn non_admin_role_fails_admin_check() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "non-admin-check", "user").await;
        let is_admin = user.role == "admin";
        assert!(!is_admin, "non-admin user should fail admin check");
        cleanup(&pool, &user.email).await;
    }

    #[tokio::test]
    async fn admin_role_passes_admin_check() {
        let Some(pool) = setup().await else { return };
        let user = create_test_user(&pool, "admin-pass-check", "admin").await;
        let is_admin = user.role == "admin";
        assert!(is_admin, "admin user should pass admin check");
        cleanup(&pool, &user.email).await;
    }
}
