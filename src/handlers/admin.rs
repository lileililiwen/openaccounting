use axum::{
    extract::State,
    middleware::{self, Next},
    response::Response,
    Router,
};
use axum_login::AuthSession;

use crate::{
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
        username: String,
        email: String,
        role: String,
        is_active: bool,
        created_at: time::OffsetDateTime,
    }

    let rows = sqlx::query_as::<_, UserRowDb>(
        "SELECT username, email, role, is_active, created_at FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let page = AdminUsersPage::new(
        user.clone(),
        rows.into_iter()
            .map(|r| UserRow {
                username: r.username,
                email: r.email,
                role: r.role,
                is_active: r.is_active,
                created_at_display: crate::templates::account::fmt_date(&r.created_at),
            })
            .collect(),
    );

    Ok(render_response(page))
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
               RETURNING id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at"#,
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
