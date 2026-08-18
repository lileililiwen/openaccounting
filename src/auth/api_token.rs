//! Per-user bearer tokens for the public REST API
//! (`a1-rest-api`).
//!
//! Token shape:
//!   `oa_live_<48 base32-ish random chars>`
//!
//! - `token_prefix`: the first 12 chars after `oa_live_`. Stored
//!   in the DB, indexed, used for O(1) lookup.
//! - `token_hash`: Argon2id hash of the full random portion
//!   (everything after `oa_live_`). Stored in the DB; the
//!   plaintext is shown to the user ONCE at creation and never
//!   persisted.
//!
//! Verification:
//!   1. Header `Authorization: Bearer oa_live_xxx` → split off
//!      the prefix `oa_live_`, derive the lookup prefix (first 12
//!      chars), look up the row.
//!   2. Verify Argon2id of the random portion matches the hash.
//!   3. If valid, return the owning `user_id`.

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

/// Public prefix of every API token. Clients can use it to
/// recognise the format (`oa_live_…`) at a glance.
pub const TOKEN_PREFIX: &str = "oa_live_";

/// Length of the secret portion of the token, in characters.
/// The secret is rendered as lower-case hex of 24 random bytes
/// (192 bits) — well above the OWASP recommendation for an
/// unguessable API key.
pub const SECRET_BYTES: usize = 24;

/// Length of the non-secret portion stored in `token_prefix`.
/// 12 hex chars = 48 bits → enough to be unique under a workload
/// where each user has at most ~100 live tokens.
pub const PREFIX_CHARS: usize = 12;

/// Row in the `api_tokens` table, with the secret hash but
/// never the plaintext.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    #[sqlx(rename = "token_hash")]
    pub _token_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A freshly-minted token. The plaintext is exposed to the user
/// exactly once (on the create-token HTTP response); the DB only
/// stores the prefix + Argon2id hash.
#[derive(Debug)]
pub struct IssuedToken {
    pub id: Uuid,
    /// Full token in `oa_live_<hex>` form. Shown to the user once.
    pub plaintext: String,
    pub row: ApiTokenRow,
}

/// Generate a new random API token, hash it, and persist it.
/// Returns the issued token (with plaintext) so the caller can
/// surface it to the user.
pub async fn issue_token(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
) -> Result<IssuedToken, sqlx::Error> {
    let secret = random_hex(SECRET_BYTES);
    let plaintext = format!("{TOKEN_PREFIX}{secret}");
    let prefix = secret[..PREFIX_CHARS].to_string();
    let hash =
        hash_secret(&secret).map_err(|e| sqlx::Error::Encode(format!("hash: {e}").into()))?;

    let row: ApiTokenRow = sqlx::query_as::<_, ApiTokenRow>(
        "INSERT INTO api_tokens (user_id, name, token_prefix, token_hash)
         VALUES ($1, $2, $3, $4)
         RETURNING id, user_id, name, token_prefix, token_hash, created_at, last_used_at, revoked_at",
    )
    .bind(user_id)
    .bind(name)
    .bind(&prefix)
    .bind(&hash)
    .fetch_one(pool)
    .await?;

    Ok(IssuedToken {
        id: row.id,
        plaintext,
        row,
    })
}

/// Verify a bearer token. Returns the `user_id` on success, or
/// `None` if the token is malformed, revoked, or unknown.
pub async fn verify_token(pool: &PgPool, header_value: &str) -> Option<Uuid> {
    let token = header_value.strip_prefix("Bearer ").unwrap_or(header_value);
    let token = token.trim();
    let secret = token.strip_prefix(TOKEN_PREFIX)?;
    if secret.len() < PREFIX_CHARS {
        return None;
    }
    let prefix = &secret[..PREFIX_CHARS];

    let row: Option<ApiTokenRow> = sqlx::query_as::<_, ApiTokenRow>(
        "SELECT id, user_id, name, token_prefix, token_hash, created_at, last_used_at, revoked_at
         FROM api_tokens
         WHERE token_prefix = $1
         LIMIT 1",
    )
    .bind(prefix)
    .fetch_optional(pool)
    .await
    .ok()?;
    let row = row?;
    if row.revoked_at.is_some() {
        return None;
    }
    if !verify_secret(secret, &row._token_hash).unwrap_or(false) {
        return None;
    }
    // Best-effort: record last_used_at. Errors here are not
    // user-visible.
    let _ = sqlx::query("UPDATE api_tokens SET last_used_at = now() WHERE id = $1")
        .bind(row.id)
        .execute(pool)
        .await;
    Some(row.user_id)
}

/// Mark a token as revoked. Idempotent — a second call on an
/// already-revoked token is a no-op.
pub async fn revoke_token(
    pool: &PgPool,
    token_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE api_tokens
         SET revoked_at = COALESCE(revoked_at, now())
         WHERE id = $1 AND user_id = $2",
    )
    .bind(token_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// List tokens for a user. Hashes are never returned to the UI
/// in clear text (we keep them for round-trip verification but
/// drop them from the public-facing struct below).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ApiTokenSummary {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_tokens(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ApiTokenSummary>, sqlx::Error> {
    sqlx::query_as::<_, ApiTokenSummary>(
        "SELECT id, name, token_prefix, created_at, last_used_at, revoked_at
         FROM api_tokens
         WHERE user_id = $1
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

// ─── internal helpers ──────────────────────────────────────────

fn random_hex(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    OsRng.fill_bytes(&mut buf);
    hex_lower(&buf)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hash_secret(secret: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2.hash_password(secret.as_bytes(), &salt)?.to_string())
}

fn verify_secret(secret: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_hex_is_the_right_length() {
        let s = random_hex(SECRET_BYTES);
        assert_eq!(s.len(), SECRET_BYTES * 2);
    }

    #[test]
    fn random_hex_is_lowercase() {
        let s = random_hex(SECRET_BYTES);
        assert!(s.chars().all(|c| !c.is_ascii_uppercase()));
    }

    #[test]
    fn hash_and_verify_round_trip() {
        let s = random_hex(SECRET_BYTES);
        let h = hash_secret(&s).unwrap();
        assert!(verify_secret(&s, &h).unwrap());
        assert!(!verify_secret("not the secret", &h).unwrap());
    }
}
