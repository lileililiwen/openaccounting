pub mod cookie_signer;
pub mod csrf;
pub mod handlers;
pub mod password;
pub mod rate_limit;
pub mod session_timeout;
pub mod totp;

use axum_login::{AuthUser, AuthnBackend, UserId};
use password::hash_password;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    #[serde(skip_serializing)]
    pub hashed_password: String,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// Theme preference: "system" | "light" | "dark".
    /// Default is "system" so legacy users keep the OS-driven
    /// behaviour until they opt in.
    pub theme: String,
    /// Locale preference: "en" | "zh-CN" | "es" | "fr" | "de" | "ja".
    /// Default is "en". Honours `Accept-Language` when unset.
    pub locale: String,
}

impl AuthUser for User {
    type Id = Uuid;
    fn id(&self) -> Uuid {
        self.id
    }
    fn session_auth_hash(&self) -> &[u8] {
        // Used to invalidate sessions on credential change. The hashed password
        // is sufficient; never expose to clients.
        self.hashed_password.as_bytes()
    }
}

#[derive(Clone, Debug)]
pub struct Backend {
    pub pool: PgPool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[async_trait::async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = AppError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let row = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
               FROM users WHERE LOWER(email) = LOWER($1)"#,
        )
        .bind(&creds.email)
        .fetch_optional(&self.pool)
        .await?;

        let Some(user) = row else {
            return Ok(None);
        };
        if !user.is_active {
            return Ok(None);
        }
        let ok = password::verify_password(&creds.password, &user.hashed_password)
            .map_err(|e| AppError::Internal(format!("password verify: {e}")))?;
        if ok {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    async fn get_user(&self, id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
               FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }
}

use crate::error::AppError;

pub async fn create_user(
    pool: &PgPool,
    email: &str,
    username: &str,
    password: &str,
) -> Result<User, AppError> {
    let hashed = hash_password(password)
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))?;
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, username, hashed_password, display_name)
           VALUES ($1, $2, $3, $2)
           RETURNING id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale"#,
    )
    .bind(email)
    .bind(username)
    .bind(&hashed)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("email or username already in use".into())
        }
        _ => AppError::Db(e),
    })?;
    Ok(user)
}

/// Change a user's password. Verifies the supplied `current` against
/// the stored hash, validates the new password, then rehashes and
/// updates the row. Returns the distinct `AppError` variant that the
/// HTTP layer maps to a 401 / 400 / 200.
pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    current: &str,
    new: &str,
) -> Result<(), AppError> {
    // 1. Load the user.
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
           FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    // 2. Verify the current password.
    let ok = password::verify_password(current, &user.hashed_password)
        .map_err(|e| AppError::Internal(format!("password verify: {e}")))?;
    if !ok {
        return Err(AppError::Unauthorized);
    }

    // 3. Validate the new password against the strength policy
    //    (length ≥ 12 + common-list check). The error message is
    //    safe to surface to the user; the password itself is never
    //    echoed.
    if let Err(e) = password::validate_strength(new) {
        return Err(AppError::Validation(e.message().into()));
    }

    // 4. Hash and store.
    let new_hash = hash_password(new)
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))?;
    sqlx::query("UPDATE users SET hashed_password = $1, updated_at = now() WHERE id = $2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for `change_password`.
    //!
    //! These are integration-style: they require a live PostgreSQL
    //! reachable via `DATABASE_URL` (same as the running app). Each
    //! test creates a unique user and cleans it up afterwards so the
    //! tests are order-independent and parallel-safe.

    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    /// Acquire a connection pool from `DATABASE_URL`, or skip the test
    /// silently if the env var is not set (e.g. during `cargo check`).
    async fn pool_or_skip() -> Option<PgPool> {
        let url = env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await
            .ok()
    }

    /// Create a user with a unique email/username and return the
    /// `User`. The `tag` argument is included in the email so
    /// parallel tests cannot collide.
    async fn make_user(pool: &PgPool, tag: &str) -> User {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let email = format!("test-{tag}-{nonce}@example.com");
        let username = format!("test_{tag}_{nonce}");
        create_user(pool, &email, &username, "old-password")
            .await
            .expect("create_user in test")
    }

    /// Remove the test user. Best-effort.
    async fn cleanup(pool: &PgPool, user_id: Uuid) {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    #[tokio::test]
    async fn change_password_rejects_wrong_current() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let user = make_user(&pool, "wrong-current").await;
        let err = change_password(&pool, user.id, "not-the-password", "new-password-1")
            .await
            .expect_err("must fail");
        assert!(matches!(err, AppError::Unauthorized), "got: {err:?}");
        // Verify stored hash did NOT change.
        let refreshed = sqlx::query_as::<_, User>(
            "SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
             FROM users WHERE id = $1",
        )
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(password::verify_password("old-password", &refreshed.hashed_password).unwrap());
        cleanup(&pool, user.id).await;
    }

    #[tokio::test]
    async fn change_password_rejects_too_short_new() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let user = make_user(&pool, "short").await;
        let err = change_password(&pool, user.id, "old-password", "short")
            .await
            .expect_err("must fail");
        match err {
            AppError::Validation(m) => assert!(m.contains("at least 12"), "msg: {m}"),
            other => panic!("expected Validation, got: {other:?}"),
        }
        cleanup(&pool, user.id).await;
    }

    #[tokio::test]
    async fn change_password_succeeds_and_persists() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let user = make_user(&pool, "happy").await;
        change_password(&pool, user.id, "old-password", "new-password-1")
            .await
            .expect("must succeed");
        let refreshed = sqlx::query_as::<_, User>(
            "SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
             FROM users WHERE id = $1",
        )
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        // New password verifies.
        assert!(password::verify_password("new-password-1", &refreshed.hashed_password).unwrap());
        // Old password no longer verifies.
        assert!(!password::verify_password("old-password", &refreshed.hashed_password).unwrap());
        cleanup(&pool, user.id).await;
    }

    #[tokio::test]
    async fn change_password_for_nonexistent_user_returns_unauthorized() {
        // Critical: the response must be indistinguishable from
        // "wrong current password" for an existing user (no
        // information disclosure).
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let err = change_password(&pool, Uuid::new_v4(), "anything", "new-password-1")
            .await
            .expect_err("must fail");
        assert!(matches!(err, AppError::Unauthorized), "got: {err:?}");
    }
}
