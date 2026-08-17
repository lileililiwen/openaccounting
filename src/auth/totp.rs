//! TOTP second-factor authentication (RFC 6238, 30 s / 6 digits / SHA-1).
//!
//! Storage
//! ────────
//! The shared secret is encrypted at rest with AES-256-GCM. The
//! encryption key is derived from `APP_SECRET` via HKDF-SHA256
//! with `info = "totp-secret-v1"`. The on-disk representation is
//! `"<b64(nonce)>:<b64(ciphertext)>"` (same shape as
//! `bank_feeds::crypto::TokenCipher`).
//!
//! Verification & replay protection
//! ────────────────────────────────
//! `last_used_counter` is the 30-second time-step counter of the
//! most recently accepted code. A new code is accepted iff:
//!
//!   * it matches the current step within the configured skew
//!     window (1 step = ±30 s, per RFC 6238 §5.2), AND
//!   * the matched step is strictly greater than
//!     `last_used_counter` (replay protection).
//!
//! Recovery codes
//! ──────────────
//! Ten 8-character base32 codes per user, Argon2id-hashed at rest.
//! A code is consumed by setting `consumed_at`; a second attempt
//! with the same code finds no unused row and is rejected.
//!
//! Public API:
//! * [`TotpCipher`]  — seal/open the TOTP secret.
//! * [`new_secret`]  — generate a fresh 20-byte secret.
//! * [`verify_code`] — verify a 6-digit code.
//! * [`generate_recovery_codes`] / [`verify_recovery_code`]
//! * [`build_otpauth_url`] — build an `otpauth://totp/...` URL.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{
    password_hash::{
        rand_core::OsRng as ArgonOsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use sqlx::PgPool;
use time::OffsetDateTime;
use totp_rs::{Algorithm, Secret, Totp};
use uuid::Uuid;

// ─── Constants ────────────────────────────────────────────────────────────

/// Number of digits in a TOTP code.
pub const DIGITS: u8 = 6;
/// Time-step in seconds.
pub const STEP: u64 = 30;
/// Skew tolerance (in steps) per RFC 6238 §5.2.
pub const SKEW: u8 = 1;
/// Number of recovery codes generated per enrollment.
pub const RECOVERY_CODE_COUNT: usize = 10;
/// Length of each recovery code (base32 characters).
pub const RECOVERY_CODE_LEN: usize = 8;
/// HKDF info string — must remain stable across releases so
/// previously-encrypted rows can still be opened.
const HKDF_INFO: &[u8] = b"totp-secret-v1";

/// Issuer name embedded in the otpauth URL so the user's
/// authenticator app displays it.
pub const ISSUER: &str = "OpenAccounting";

// ─── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TotpError {
    #[error("invalid code")]
    InvalidCode,
    #[error("not enrolled")]
    NotEnrolled,
    #[error("encryption failed")]
    Encryption,
    #[error("decryption failed")]
    Decryption,
    #[error("base32 secret error: {0}")]
    Base32(String),
    #[error("totp-rs error: {0}")]
    TotpLib(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

// ─── Cipher (AES-256-GCM keyed via HKDF) ─────────────────────────────────

/// Encrypts / decrypts the TOTP secret at rest.
///
/// The key is derived once from `APP_SECRET` via HKDF-SHA256 so
/// rotating `APP_SECRET` invalidates every existing secret.
/// Existing rows must be re-enrolled by users — acceptable for a
/// feature-flag rollout.
#[derive(Clone)]
pub struct TotpCipher {
    key: [u8; 32],
}

impl TotpCipher {
    /// Build the cipher from the application secret. The secret
    /// is the existing `APP_SECRET` (≥ 32 chars per the
    /// configuration layer's invariant); HKDF expands it to 32
    /// bytes regardless of input length.
    pub fn from_app_secret(app_secret: &str) -> Self {
        let hk = Hkdf::<Sha256>::new(None, app_secret.as_bytes());
        let mut okm = [0u8; 32];
        hk.expand(HKDF_INFO, &mut okm)
            .expect("invariant: HKDF expand to 32 bytes is well-defined");
        Self { key: okm }
    }

    /// Encrypt `plaintext`. Returns `"<b64_nonce>:<b64_ciphertext>"`.
    pub fn seal(&self, plaintext: &str) -> Result<String, TotpError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| TotpError::Encryption)?;
        Ok(format!("{}:{}", B64.encode(nonce), B64.encode(ct)))
    }

    /// Decrypt a blob produced by [`seal`].
    pub fn open(&self, blob: &str) -> Result<String, TotpError> {
        let (nonce_b64, ct_b64) = blob.split_once(':').ok_or(TotpError::Decryption)?;
        let nonce_bytes = B64.decode(nonce_b64).map_err(|_| TotpError::Decryption)?;
        let ct_bytes = B64.decode(ct_b64).map_err(|_| TotpError::Decryption)?;
        if nonce_bytes.len() != 12 {
            return Err(TotpError::Decryption);
        }
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let pt = cipher
            .decrypt(nonce, ct_bytes.as_slice())
            .map_err(|_| TotpError::Decryption)?;
        String::from_utf8(pt).map_err(|_| TotpError::Decryption)
    }
}

// ─── Secret generation ──────────────────────────────────────────────────

/// Generate a fresh 20-byte TOTP secret, returned as base32.
///
/// 20 bytes (160 bits) is the recommended secret length for
/// SHA-1-based TOTP and is compatible with Google Authenticator,
/// 1Password, Bitwarden, Authy, etc.
pub fn new_secret() -> Result<String, TotpError> {
    let mut bytes = [0u8; 20];
    OsRng.fill_bytes(&mut bytes);
    Ok(Secret::from(bytes.to_vec()).to_base32())
}

// ─── TOTP wrapper for verification ───────────────────────────────────────

/// Build a [`Totp`] from a base32-encoded secret.
fn totp_from_secret(secret_b32: &str) -> Result<Totp, TotpError> {
    let secret =
        Secret::try_from_base32(secret_b32).map_err(|e| TotpError::Base32(format!("{e:?}")))?;
    totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(DIGITS)
        .with_skew(u16::from(SKEW))
        .with_step_duration(STEP)
        .with_secret(secret)
        .build()
        .map_err(|e| TotpError::TotpLib(format!("{e:?}")))
}

/// Build an `otpauth://totp/...` URL the user can paste into
/// their authenticator app. The issuer is `OpenAccounting` and
/// the account label is the user's email.
pub fn build_otpauth_url(email: &str, secret_b32: &str) -> Result<String, TotpError> {
    let secret =
        Secret::try_from_base32(secret_b32).map_err(|e| TotpError::Base32(format!("{e:?}")))?;
    let totp = totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(DIGITS)
        .with_skew(u16::from(SKEW))
        .with_step_duration(STEP)
        .with_secret(secret)
        .with_account_name(email)
        .with_issuer(Some(ISSUER))
        .build()
        .map_err(|e| TotpError::TotpLib(format!("{e:?}")))?;
    totp.to_url()
        .map_err(|e| TotpError::TotpLib(format!("{e:?}")))
}

/// Verify a 6-digit code against a stored secret.
///
/// `last_used_counter` is the step counter of the most recently
/// accepted code (or 0 if none). The current step is computed
/// from `now`. The code is accepted iff:
///
/// 1. it matches a step within the configured skew window, AND
/// 2. the matched step is strictly greater than
///    `last_used_counter` (replay protection).
///
/// On success, returns the matched step value the caller MUST
/// persist (atomically) to lock out replays.
pub fn verify_code(
    secret_b32: &str,
    code: &str,
    last_used_counter: i64,
    now: OffsetDateTime,
) -> Result<u64, TotpError> {
    let totp = totp_from_secret(secret_b32)?;
    let now_u64 = now.unix_timestamp() as u64;
    let Some(matched_step) = totp.check(code, now_u64) else {
        return Err(TotpError::InvalidCode);
    };
    let matched_i64 = matched_step as i64;
    if matched_i64 <= last_used_counter {
        return Err(TotpError::InvalidCode);
    }
    Ok(matched_step)
}

// ─── Recovery codes ─────────────────────────────────────────────────────

/// Generate a set of fresh recovery codes. Returns the plaintext
/// codes — the caller is responsible for showing them ONCE and
/// storing only the Argon2id hashes.
pub fn generate_recovery_codes() -> Vec<String> {
    // 8 chars of base32 (5 bits each) → 40 bits of entropy. The
    // user has 10 codes, so total entropy is ~400 bits, which
    // is far above the practical attack threshold for offline
    // guessing.
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let alphabet_len = alphabet.len();
    (0..RECOVERY_CODE_COUNT)
        .map(|_| {
            (0..RECOVERY_CODE_LEN)
                .map(|_| {
                    let idx = (rand::random::<u8>() as usize) % alphabet_len;
                    alphabet[idx] as char
                })
                .collect()
        })
        .collect()
}

/// Argon2id-hash a single recovery code so we can store it.
pub fn hash_recovery_code(code: &str) -> Result<String, TotpError> {
    let salt = SaltString::generate(&mut ArgonOsRng);
    Argon2::default()
        .hash_password(code.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| TotpError::Internal(format!("argon2: {e}")))
}

/// Verify a recovery code against a stored Argon2id hash.
pub fn verify_recovery_code_hash(code: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(code.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Try to consume one unused recovery code for `user_id`.
///
/// On success, returns the [`uuid::Uuid`] of the consumed row.
/// On failure (no matching unused code), returns
/// `Err(TotpError::InvalidCode)`.
///
/// `pool` is used for a single SELECT … FOR UPDATE SKIP LOCKED
/// + UPDATE in a transaction so two concurrent 2FA submissions
/// can't both redeem the same code.
pub async fn consume_recovery_code(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
) -> Result<Uuid, TotpError> {
    let mut tx = pool.begin().await?;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT id, code_hash FROM recovery_codes
           WHERE user_id = $1 AND consumed_at IS NULL
           FOR UPDATE SKIP LOCKED"#,
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    for (id, hash) in rows {
        if verify_recovery_code_hash(code, &hash) {
            sqlx::query(r#"UPDATE recovery_codes SET consumed_at = now() WHERE id = $1"#)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(id);
        }
    }
    tx.rollback().await.ok();
    Err(TotpError::InvalidCode)
}

// ─── Persistence helpers ────────────────────────────────────────────────

/// Has the user enrolled TOTP?
pub async fn is_enrolled(pool: &PgPool, user_id: Uuid) -> Result<bool, TotpError> {
    let row: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT user_id FROM user_totp WHERE user_id = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

/// Look up the encrypted secret and last-used counter for a user.
/// Returns `Ok(None)` when the user has not enrolled.
pub async fn fetch_state(pool: &PgPool, user_id: Uuid) -> Result<Option<(String, i64)>, TotpError> {
    let row: Option<(String, i64)> = sqlx::query_as(
        r#"SELECT secret_encrypted, last_used_counter
           FROM user_totp WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Atomically update `last_used_counter` iff the supplied expected
/// value still matches. Returns true iff this caller actually
/// advanced the counter (used to defeat races between two
/// concurrent verify calls).
pub async fn advance_counter(
    pool: &PgPool,
    user_id: Uuid,
    expected: i64,
    new_value: i64,
) -> Result<bool, TotpError> {
    let res = sqlx::query(
        r#"UPDATE user_totp SET last_used_counter = $1
           WHERE user_id = $2 AND last_used_counter = $3"#,
    )
    .bind(new_value)
    .bind(user_id)
    .bind(expected)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// Persist a brand-new TOTP enrollment (encrypted secret).
pub async fn enroll(pool: &PgPool, user_id: Uuid, secret_encrypted: &str) -> Result<(), TotpError> {
    sqlx::query(
        r#"INSERT INTO user_totp (user_id, secret_encrypted)
           VALUES ($1, $2)
           ON CONFLICT (user_id) DO UPDATE
             SET secret_encrypted = EXCLUDED.secret_encrypted,
                 enrolled_at = now(),
                 last_used_counter = 0"#,
    )
    .bind(user_id)
    .bind(secret_encrypted)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove TOTP enrollment and ALL recovery codes for the user.
pub async fn disable(pool: &PgPool, user_id: Uuid) -> Result<(), TotpError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM user_totp WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Wipe existing recovery codes and insert a fresh batch of
/// Argon2id hashes. Returns the plaintext codes so the caller can
/// render them in the UI once.
pub async fn regenerate_recovery_codes(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<String>, TotpError> {
    let codes = generate_recovery_codes();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for code in &codes {
        let hash = hash_recovery_code(code)?;
        sqlx::query(
            r#"INSERT INTO recovery_codes (user_id, code_hash)
               VALUES ($1, $2)"#,
        )
        .bind(user_id)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(codes)
}

/// Count the unused recovery codes for a user (used by the
/// settings page to warn "X codes remaining").
pub async fn unused_recovery_code_count(pool: &PgPool, user_id: Uuid) -> Result<i64, TotpError> {
    let (n,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM recovery_codes
           WHERE user_id = $1 AND consumed_at IS NULL"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

// ─── QR-code rendering (server-side SVG) ────────────────────────────────

/// Render an SVG QR code from a string. `border` is the number of
/// empty modules around the code; 4 is standard.
///
/// Uses the pure-Rust `qrcodegen` crate; no JS or canvas
/// dependency required. The returned SVG is suitable for
/// inlining into an Askama template.
pub fn qr_svg(text: &str, border: i32) -> Result<String, TotpError> {
    use qrcodegen::{QrCode, QrCodeEcc};
    let code = QrCode::encode_text(text, QrCodeEcc::Medium)
        .map_err(|e| TotpError::Internal(format!("qr: {e:?}")))?;
    let n = code.size();
    let total = n + 2 * border;
    let mut out = String::with_capacity(4096);
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" viewBox=\"0 0 {total} {total}\" shape-rendering=\"crispEdges\">",
    ));
    out.push_str("<rect width=\"100%\" height=\"100%\" fill=\"white\"/>");
    out.push_str("<path d=\"");
    for y in 0..n {
        for x in 0..n {
            if code.get_module(x, y) {
                let px = x + border;
                let py = y + border;
                out.push_str(&format!("M{px} {py}h1v1h-1z"));
            }
        }
    }
    out.push_str("\" fill=\"black\"/></svg>");
    Ok(out)
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    async fn pool_or_skip() -> Option<PgPool> {
        let url = env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .ok()
    }

    fn uniq_id(tag: &str) -> Uuid {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!("totp-test-{tag}-{nanos}");
        Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes())
    }

    #[test]
    fn cipher_seal_open_roundtrip() {
        let c = TotpCipher::from_app_secret(
            "test-secret-do-not-use-in-production-please-replace-with-64-random-chars",
        );
        let plain = "JBSWY3DPEHPK3PXP";
        let sealed = c.seal(plain).expect("seal");
        let opened = c.open(&sealed).expect("open");
        assert_eq!(opened, plain);
    }

    #[test]
    fn cipher_open_bad_blob_returns_error() {
        let c = TotpCipher::from_app_secret("x".repeat(32).as_str());
        assert!(matches!(c.open("nope"), Err(TotpError::Decryption)));
        assert!(matches!(c.open(""), Err(TotpError::Decryption)));
        assert!(matches!(c.open("a:b:c"), Err(TotpError::Decryption)));
    }

    #[test]
    fn totp_code_verifies_against_secret() {
        let secret_b32 = new_secret().expect("generate");
        let totp = totp_from_secret(&secret_b32).expect("totp");
        let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
        let code = totp.generate(now).to_string();
        let accepted =
            verify_code(&secret_b32, &code, 0, OffsetDateTime::now_utc()).expect("verify");
        assert!(accepted > 0, "advanced counter must be positive");
    }

    #[test]
    fn totp_old_code_rejected_after_use() {
        let secret_b32 = new_secret().expect("generate");
        let totp = totp_from_secret(&secret_b32).expect("totp");
        let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
        let code = totp.generate(now).to_string();
        // First verification succeeds.
        let advanced =
            verify_code(&secret_b32, &code, 0, OffsetDateTime::now_utc()).expect("first verify");
        // Replay at the same step fails because the expected
        // counter is now equal to (not strictly less than) the
        // matched step.
        let err = verify_code(
            &secret_b32,
            &code,
            advanced as i64,
            OffsetDateTime::now_utc(),
        )
        .expect_err("replay must fail");
        assert!(matches!(err, TotpError::InvalidCode));
    }

    #[test]
    fn totp_recovery_code_one_shot() {
        let codes = generate_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODE_COUNT);
        assert!(codes.iter().all(|c| c.len() == RECOVERY_CODE_LEN));
        let hash = hash_recovery_code(&codes[0]).expect("hash");
        assert!(verify_recovery_code_hash(&codes[0], &hash));
        assert!(!verify_recovery_code_hash(&codes[1], &hash));
        assert!(!verify_recovery_code_hash("XXXXXXXX", &hash));
    }

    #[test]
    fn otpauth_url_includes_issuer_and_label() {
        let secret_b32 = new_secret().expect("generate");
        let url = build_otpauth_url("alice@example.com", &secret_b32).expect("url");
        assert!(url.starts_with("otpauth://totp/"), "got: {url}");
        assert!(url.contains("OpenAccounting"), "issuer missing in {url}");
        assert!(
            url.contains("alice%40example.com") || url.contains("alice@example.com"),
            "label missing in {url}"
        );
        assert!(url.contains("secret="), "secret param missing in {url}");
    }

    #[test]
    fn qr_svg_renders_non_empty_svg() {
        let svg = qr_svg("otpauth://totp/test:user?secret=JBSWY3DPEHPK3PXP", 4).expect("render");
        assert!(svg.starts_with("<svg "), "got: {svg}");
        assert!(svg.contains("<path"), "QR path missing");
        assert!(svg.contains("</svg>"));
    }

    #[tokio::test]
    async fn db_enroll_disable_roundtrip() {
        let Some(pool) = pool_or_skip().await else {
            return;
        };
        let user_id = uniq_id("db-roundtrip");

        // user_totp FKs users; create a stub user first.
        sqlx::query(
            "INSERT INTO users (id, email, username, hashed_password, display_name)
             VALUES ($1, 'totp-test@example.com', 'totp_test',
                     '$argon2id$v=19$m=19456,t=2,p=1$YWFhYWFhYWFhYWFh$placeholder',
                     'Totp Test')
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("create stub user");

        let cipher = TotpCipher::from_app_secret(
            "test-secret-do-not-use-in-production-please-replace-with-64-random-chars",
        );
        let secret = new_secret().expect("secret");
        let sealed = cipher.seal(&secret).expect("seal");

        enroll(&pool, user_id, &sealed).await.expect("enroll");
        assert!(is_enrolled(&pool, user_id).await.expect("is_enrolled"));

        let codes = regenerate_recovery_codes(&pool, user_id)
            .await
            .expect("regen");
        assert_eq!(codes.len(), RECOVERY_CODE_COUNT);

        // Consume one recovery code.
        let _consumed = consume_recovery_code(&pool, user_id, &codes[0])
            .await
            .expect("consume");
        // Second attempt with the same code fails.
        let err = consume_recovery_code(&pool, user_id, &codes[0])
            .await
            .expect_err("second consume must fail");
        assert!(matches!(err, TotpError::InvalidCode));

        // Disable: secret and codes gone.
        disable(&pool, user_id).await.expect("disable");
        assert!(!is_enrolled(&pool, user_id).await.expect("post-disable"));
        let remaining = unused_recovery_code_count(&pool, user_id)
            .await
            .expect("count");
        assert_eq!(remaining, 0);

        // Cleanup
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await
            .ok();
    }
}
