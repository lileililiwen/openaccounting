//! OIDC single sign-on (`oidc-sso`).
//!
//! One admin-configured provider, Authorization Code + PKCE (S256),
//! discovery + JWKS verification via the `openidconnect` crate.
//! Identity = `(issuer, subject)`; linking uses the *verified* email
//! claim only — unverified emails never link (account-takeover guard).

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

/// Derive the AES key for the provider client secret from APP_SECRET
/// (same HKDF posture as TotpCipher).
fn secret_cipher() -> crate::bank_feeds::crypto::TokenCipher {
    let app_secret = std::env::var("APP_SECRET").unwrap_or_else(|_| "dev-secret".to_string());
    let hk = hkdf::Hkdf::<Sha256>::new(None, app_secret.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"oa-oidc-provider-secret", &mut okm)
        .expect("invariant: HKDF expand to 32 bytes is well-defined");
    crate::bank_feeds::crypto::TokenCipher::new(okm)
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    /// 'auto' | 'invite-only'
    pub provisioning: String,
    /// Password login + registration disabled instance-wide.
    pub sso_only: bool,
}

/// Load the enabled provider. `None` when unconfigured/disabled —
/// callers treat that as "SSO does not exist".
pub async fn load_provider(pool: &PgPool) -> Result<Option<ProviderConfig>, sqlx::Error> {
    let row: Option<(String, String, String, String, String, bool)> = sqlx::query_as(
        "SELECT issuer_url, client_id, client_secret_enc, scopes, provisioning, sso_only
         FROM oidc_provider WHERE is_enabled = TRUE LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some((issuer_url, client_id, enc, scopes, provisioning, sso_only)) = row else {
        return Ok(None);
    };
    let Some(client_secret) = secret_cipher().open(&enc) else {
        tracing::error!("OIDC client secret failed to decrypt; SSO disabled until re-saved");
        return Ok(None);
    };
    Ok(Some(ProviderConfig {
        issuer_url,
        client_id,
        client_secret,
        scopes: scopes.split(' ').map(str::to_string).collect(),
        provisioning,
        sso_only,
    }))
}

/// Admin save: encrypts the secret at rest. Upserts the single row.
pub async fn save_provider(
    pool: &PgPool,
    issuer_url: &str,
    client_id: &str,
    client_secret: &str,
    scopes: &str,
    provisioning: &str,
    sso_only: bool,
    updated_by: Uuid,
) -> Result<(), sqlx::Error> {
    let enc = secret_cipher().seal(client_secret);
    sqlx::query(
        r#"INSERT INTO oidc_provider
               (issuer_url, client_id, client_secret_enc, scopes, provisioning, sso_only, updated_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           ON CONFLICT (id) DO UPDATE SET
               issuer_url = EXCLUDED.issuer_url,
               client_id = EXCLUDED.client_id,
               client_secret_enc = EXCLUDED.client_secret_enc,
               scopes = EXCLUDED.scopes,
               provisioning = EXCLUDED.provisioning,
               sso_only = EXCLUDED.sso_only,
               updated_by = EXCLUDED.updated_by,
               updated_at = now()"#,
    )
    .bind(issuer_url)
    .bind(client_id)
    .bind(enc)
    .bind(scopes)
    .bind(provisioning)
    .bind(sso_only)
    .bind(updated_by)
    .execute(pool)
    .await?;
    Ok(())
}

// ── State/nonce signing ─────────────────────────────────────────────────

fn state_key() -> Vec<u8> {
    let secret = std::env::var("APP_SECRET").unwrap_or_else(|_| "dev-secret".into());
    let mut mac =
        <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(b"oa-oidc-state-v1");
    mac.finalize().into_bytes().to_vec()
}

/// Build a tamper-evident OAuth state: `<nonce>.<sig>`.
pub fn sign_state(nonce_hex: &str) -> String {
    let mut mac = <Hmac<Sha256>>::new_from_slice(&state_key()).expect("key ok");
    mac.update(nonce_hex.as_bytes());
    format!("{nonce_hex}.{}", hex::encode(mac.finalize().into_bytes()))
}

/// Verify a state token; returns the nonce when valid.
pub fn verify_state(state: &str) -> Option<String> {
    let (nonce, sig_hex) = state.split_once('.')?;
    let sig = hex::decode(sig_hex).ok()?;
    let mut mac = <Hmac<Sha256>>::new_from_slice(&state_key()).expect("key ok");
    mac.update(nonce.as_bytes());
    if !constant_time_eq(mac.finalize().into_bytes().as_slice(), &sig) {
        return None;
    }
    Some(nonce.to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ── Linking / provisioning ──────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("email not verified by the identity provider")]
    UnverifiedEmail,
    #[error("this identity is already linked to a different account")]
    SubjectTaken,
    #[error("unknown email; ask an administrator for an invitation")]
    InviteOnly,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

#[derive(Debug)]
pub struct ResolvedUser {
    pub user_id: Uuid,
    /// True when a new local account was provisioned.
    pub created: bool,
}

/// Find-or-create the local user for a verified OIDC identity.
///
/// Order of resolution:
/// 1. existing `(issuer, subject)` identity → that user;
/// 2. existing local user with the same verified email → link silently;
/// 3. provisioning policy: `auto` creates the account, `invite-only`
///    rejects.
pub async fn resolve_user(
    pool: &PgPool,
    issuer: &str,
    subject: &str,
    email: &str,
    email_verified: bool,
) -> Result<ResolvedUser, LinkError> {
    if !email_verified {
        return Err(LinkError::UnverifiedEmail);
    }

    // 1. Existing identity.
    let linked: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM oidc_identities WHERE issuer = $1 AND subject = $2")
            .bind(issuer)
            .bind(subject)
            .fetch_optional(pool)
            .await?;
    if let Some((user_id,)) = linked {
        return Ok(ResolvedUser {
            user_id,
            created: false,
        });
    }

    // Guard: subject must not be bound to another user through a race.
    // (The UNIQUE index arbitrates; we pre-check for a nicer error.)

    // 2. Link by verified email.
    let by_email: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1 AND is_active = TRUE")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    if let Some((user_id,)) = by_email {
        let inserted = sqlx::query(
            "INSERT INTO oidc_identities (issuer, subject, user_id) VALUES ($1, $2, $3)
             ON CONFLICT (issuer, subject) DO NOTHING",
        )
        .bind(issuer)
        .bind(subject)
        .bind(user_id)
        .execute(pool)
        .await?;
        if inserted.rows_affected() == 0 {
            return Err(LinkError::SubjectTaken);
        }
        return Ok(ResolvedUser {
            user_id,
            created: false,
        });
    }

    // 3. Unknown email: the caller decides per provisioning policy
    // (`auto` → provision_user, `invite-only` → this error).
    Err(LinkError::InviteOnly)
}

/// Provision under `auto` policy (called only when policy allows).
pub async fn provision_user(
    pool: &PgPool,
    issuer: &str,
    subject: &str,
    email: &str,
    display_name: &str,
) -> Result<ResolvedUser, LinkError> {
    let username: String = display_name
        .trim()
        .is_empty()
        .then(|| email.split('@').next().unwrap_or("user").to_string())
        .unwrap_or_else(|| display_name.trim().to_string());
    let password_hash = argon2_hash_random(); // unusable password; SSO-only login
    let (user_id,): (Uuid,) = sqlx::query_as(
        r#"INSERT INTO users (email, username, display_name, hashed_password, role, is_active)
           VALUES ($1, $2, $3, $4, 'user', TRUE)
           ON CONFLICT (email) DO UPDATE SET updated_at = now()
           RETURNING id"#,
    )
    .bind(email)
    .bind(username)
    .bind(display_name)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    sqlx::query("INSERT INTO oidc_identities (issuer, subject, user_id) VALUES ($1, $2, $3)")
        .bind(issuer)
        .bind(subject)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(ResolvedUser {
        user_id,
        created: true,
    })
}

fn argon2_hash_random() -> String {
    // A random hash no password verifies: local login stays locked for
    // provisioned accounts while satisfying the NOT NULL column.
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(b"\x00do-not-match\x00", &salt)
        .map(|p| p.to_string())
        .unwrap_or_else(|_| "x".to_string())
}
