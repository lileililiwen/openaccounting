//! HTTP handlers for TOTP enrollment, code regeneration, and
//! disable. All routes require an authenticated user; the
//! protected router layer enforces that.
//!
//! Routes:
//!   GET  /account/security              — show enroll / management page
//!   POST /account/security/enroll       — confirm a 6-digit code &
//!                                          finalize enrollment
//!   POST /account/security/disable      — disable 2FA (needs password
//!                                          + code)
//!   POST /account/security/regen-codes  — issue a fresh batch of
//!                                          recovery codes (needs
//!                                          password + code)

use askama::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form, Router,
};
use axum_login::AuthSession;
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    auth::{
        password,
        totp::{
            self, build_otpauth_url, qr_svg, verify_code, TotpCipher, TotpError,
            RECOVERY_CODE_COUNT,
        },
        Backend, User,
    },
    error::{AppError, AppResult},
    templates::{account::SecurityPage, render_response},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/security", axum::routing::get(security_page))
        .route(
            "/account/security/enroll",
            axum::routing::post(enroll_submit),
        )
        .route(
            "/account/security/disable",
            axum::routing::post(disable_submit),
        )
        .route(
            "/account/security/regen-codes",
            axum::routing::post(regen_codes_submit),
        )
}

// ─── Helpers ────────────────────────────────────────────────────────────

async fn require_user(auth: &AuthSession<Backend>) -> AppResult<(Uuid, String, String)> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();
    Ok((user.id, user.email, user.hashed_password))
}

async fn verify_password_or_unauthorized(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    candidate: &str,
) -> AppResult<()> {
    let user: User = sqlx::query_as(
        r#"SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
           FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::Unauthorized)?;
    let ok = password::verify_password(candidate, &user.hashed_password)
        .map_err(|e| AppError::Internal(format!("password: {e}")))?;
    if !ok {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

async fn verify_totp_or_invalid(
    cipher: &TotpCipher,
    pool: &sqlx::PgPool,
    user_id: Uuid,
    code: &str,
) -> AppResult<u64> {
    let Some((sealed, counter)) = totp::fetch_state(pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?
    else {
        return Err(AppError::Validation("2FA is not enrolled".into()));
    };
    let secret = cipher
        .open(&sealed)
        .map_err(|e| AppError::Internal(format!("cipher: {e:?}")))?;
    verify_code(&secret, code, counter, OffsetDateTime::now_utc()).map_err(|e| match e {
        TotpError::InvalidCode => AppError::Unauthorized,
        other => AppError::Internal(format!("totp: {other:?}")),
    })
}

// ─── GET /account/security ──────────────────────────────────────────────

pub async fn security_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();
    let pool = &state.pool;

    if !totp::is_enrolled(pool, user.id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?
    {
        let secret = totp::new_secret()
            .map_err(|e| AppError::Internal(format!("totp new_secret: {e:?}")))?;
        let url = build_otpauth_url(&user.email, &secret)
            .map_err(|e| AppError::Internal(format!("otpauth: {e:?}")))?;
        let qr = qr_svg(&url, 4).map_err(|e| AppError::Internal(format!("qr: {e:?}")))?;
        Ok(render_response(SecurityPage::not_enrolled(
            qr, secret, &user,
        )))
    } else {
        let row: Option<(OffsetDateTime,)> =
            sqlx::query_as("SELECT enrolled_at FROM user_totp WHERE user_id = $1")
                .bind(user.id)
                .fetch_optional(pool)
                .await
                .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
        let enrolled_at = row
            .map(|(t,)| crate::templates::account::fmt_date(&t))
            .unwrap_or_default();
        let unused = totp::unused_recovery_code_count(pool, user.id)
            .await
            .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
        Ok(render_response(SecurityPage::enrolled(
            enrolled_at,
            unused,
            Vec::new(),
            &user,
        )))
    }
}

// ─── POST /account/security/enroll ──────────────────────────────────────

#[derive(Deserialize)]
pub struct EnrollForm {
    pub code: String,
    /// Base32 secret round-tripped from the GET page in a hidden
    /// form field. This is the standard pattern for non-JS 2FA
    /// enrollment flows: the server never persists the secret
    /// until the user proves they scanned it by submitting a
    /// valid code.
    pub secret: String,
}

pub async fn enroll_submit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<EnrollForm>,
) -> AppResult<Response> {
    let (user_id, _, _) = require_user(&auth).await?;

    // Verify the code against the supplied secret. On failure, we
    // re-render the enrollment page with a fresh secret + error.
    let now = OffsetDateTime::now_utc();
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();
    if verify_code(&form.secret, &form.code, 0, now).is_err() {
        let secret = totp::new_secret()
            .map_err(|e| AppError::Internal(format!("totp new_secret: {e:?}")))?;
        let url = build_otpauth_url(&user.email, &secret)
            .map_err(|e| AppError::Internal(format!("otpauth: {e:?}")))?;
        let qr = qr_svg(&url, 4).map_err(|e| AppError::Internal(format!("qr: {e:?}")))?;
        let page = SecurityPage::not_enrolled(qr, secret, &user)
            .with_error("Invalid 6-digit code; please try again.");
        return Ok(render_response(page));
    }

    // Persist the encrypted secret.
    let sealed = state
        .totp_cipher
        .seal(&form.secret)
        .map_err(|e| AppError::Internal(format!("cipher seal: {e:?}")))?;
    totp::enroll(&state.pool, user_id, &sealed)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
    let codes = totp::regenerate_recovery_codes(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;

    let (enrolled_at,): (OffsetDateTime,) =
        sqlx::query_as("SELECT enrolled_at FROM user_totp WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
    let unused: i64 = totp::unused_recovery_code_count(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))? as i64;

    let page = SecurityPage::enrolled(
        crate::templates::account::fmt_date(&enrolled_at),
        unused,
        codes,
        &user,
    )
    .with_flash("Two-factor authentication enabled. Save your recovery codes now — they will not be shown again.");
    Ok(render_response(page))
}

// ─── POST /account/security/disable ─────────────────────────────────────

#[derive(Deserialize)]
pub struct DisableForm {
    pub password: String,
    pub code: String,
}

pub async fn disable_submit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<DisableForm>,
) -> AppResult<Response> {
    let (user_id, _, _) = require_user(&auth).await?;
    // Password check first — if the password is wrong we reject
    // without invoking the TOTP path so the response time does
    // not leak whether 2FA was enrolled.
    verify_password_or_unauthorized(&state.pool, user_id, &form.password).await?;
    // 2FA must be enrolled for the disable path to make sense.
    let Some(_) = totp::fetch_state(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?
    else {
        // Spec: "Disable from disabled account is a no-op 200."
        return Ok(Redirect::to("/account/security").into_response());
    };
    // TOTP code must be valid.
    let _matched_step =
        verify_totp_or_invalid(&state.totp_cipher, &state.pool, user_id, &form.code).await?;

    totp::disable(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
    Ok(Redirect::to("/account/security").into_response())
}

// ─── POST /account/security/regen-codes ─────────────────────────────────

pub async fn regen_codes_submit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<DisableForm>,
) -> AppResult<Response> {
    let (user_id, _, _) = require_user(&auth).await?;
    verify_password_or_unauthorized(&state.pool, user_id, &form.password).await?;
    let _matched_step =
        verify_totp_or_invalid(&state.totp_cipher, &state.pool, user_id, &form.code).await?;

    let codes = totp::regenerate_recovery_codes(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;

    let (enrolled_at,): (OffsetDateTime,) =
        sqlx::query_as("SELECT enrolled_at FROM user_totp WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?;
    let unused: i64 = RECOVERY_CODE_COUNT as i64;
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();
    let page = SecurityPage::enrolled(
        crate::templates::account::fmt_date(&enrolled_at),
        unused,
        codes,
        &user,
    )
    .with_flash("Recovery codes regenerated. The previous codes are no longer valid.");
    Ok(render_response(page))
}

// ─── Tests ──────────────────────────────────────────────────────────────
// Behavioural tests live in tests/integration/totp.rs so they
// exercise the full HTTP stack.
