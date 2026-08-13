pub mod handlers;
pub mod password;

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
    #[serde(skip_serializing)]
    pub hashed_password: String,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
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
            r#"SELECT id, email, username, display_name, hashed_password, is_active, created_at, updated_at
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
            r#"SELECT id, email, username, display_name, hashed_password, is_active, created_at, updated_at
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
           RETURNING id, email, username, display_name, hashed_password, is_active, created_at, updated_at"#,
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
