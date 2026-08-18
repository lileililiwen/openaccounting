use crate::templates::{auth::Login2faPage, render_response};
use crate::{
    auth::{
        create_user, password,
        rate_limit::{self, Decision},
        session_timeout::mark_authenticated,
        totp, Backend, Credentials,
    },
    error::{AppError, AppResult},
    AppState,
};
use askama::Template;
use axum::{
    extract::ConnectInfo,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use axum_login::AuthSession;
use serde::Deserialize;
use std::net::SocketAddr;
use time::OffsetDateTime;
use tower_sessions::Session;
use uuid::Uuid;

/// Session key for the user_id that's pending 2FA verification.
const SESSION_KEY_2FA_PENDING: &str = "2fa_pending_user_id";
/// Session key for the `next` URL to redirect to after a
/// successful 2FA.
const SESSION_KEY_2FA_NEXT: &str = "2fa_next";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", axum::routing::get(login_page).post(login_submit))
        .route(
            "/login/2fa",
            axum::routing::get(login_2fa_page).post(login_2fa_submit),
        )
        .route(
            "/register",
            axum::routing::get(register_page).post(register_submit),
        )
        .route("/logout", axum::routing::post(logout))
}

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginPage {
    pub error: String,
    pub next: String,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct RegisterPage {
    pub error: String,
}

pub async fn login_page(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let error = if params.get("expired").map(|s| s.as_str()) == Some("1") {
        "Your session expired; please sign in again."
    } else {
        ""
    };
    Ok(render_response(LoginPage {
        error: error.to_string(),
        next: params.get("next").cloned().unwrap_or_default(),
    }))
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub next: Option<String>,
}

/// Generic login error used for both wrong-password and
/// rate-limit rejections. The text is identical on purpose so
/// attackers cannot enumerate valid emails (per the
/// `s2-login-rate-limiting` spec).
const GENERIC_LOGIN_ERROR: &str = "Invalid email or password";

/// Render the generic login error page at the requested status.
///
/// `status` is the HTTP status code to return. Wrong-password uses
/// 200 (the login page renders normally). Rate-limited rejections
/// use 429 so the user sees the throttle signal — but the rendered
/// HTML body is byte-equal so attackers cannot tell from the body
/// whether the email is valid.
fn login_error_page(next: &str, status: StatusCode) -> Response {
    let page = LoginPage {
        error: GENERIC_LOGIN_ERROR.into(),
        next: next.to_string(),
    };
    match page.render() {
        Ok(body) => (
            status,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Template error: {err}"),
        )
            .into_response(),
    }
}

pub async fn login_submit(
    mut auth: AuthSession<Backend>,
    session: Session,
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Form(form): axum::Form<LoginForm>,
) -> AppResult<Response> {
    let now = OffsetDateTime::now_utc();
    let ip = peer.ip().to_string();
    let email_norm = rate_limit::normalise_email(&form.email);
    let next = form.next.clone().unwrap_or_default();

    // ── Throttle gate ────────────────────────────────────────────────────────
    // We check both throttles BEFORE running Argon2id. Argon2id is
    // intentionally slow (~50-200 ms); a successful throttle check
    // here is what protects the CPU budget. The generic error page
    // is intentionally indistinguishable from the wrong-password
    // page so attackers cannot enumerate valid emails.
    let account_ok = rate_limit::check_account(&state.pool, &email_norm, now)
        .await
        .map_err(AppError::Db)?;
    if account_ok == Decision::Throttled {
        tracing::warn!(ip = %ip, email = %email_norm, "login throttled (account)");
        return Ok(login_error_page(&next, StatusCode::TOO_MANY_REQUESTS));
    }
    let ip_ok = rate_limit::check_ip(&state.pool, &ip, now)
        .await
        .map_err(AppError::Db)?;
    if ip_ok == Decision::Throttled {
        tracing::warn!(ip = %ip, "login throttled (ip)");
        return Ok(login_error_page(&next, StatusCode::TOO_MANY_REQUESTS));
    }

    // ── Authentication ──────────────────────────────────────────────────────
    let creds = Credentials {
        email: form.email.clone(),
        password: form.password.clone(),
    };
    let user = match auth
        .authenticate(creds)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?
    {
        Some(u) => u,
        None => {
            rate_limit::record_attempt(&state.pool, &ip, &email_norm, false, now)
                .await
                .map_err(AppError::Db)?;
            return Ok(login_error_page(&next, StatusCode::OK));
        }
    };

    // Password verified. Now: if the user has 2FA enrolled,
    // stash the user_id in the session and require a TOTP
    // code at /login/2fa before we materialise the session.
    // Otherwise the user is logged in directly.
    if totp::is_enrolled(&state.pool, user.id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?
    {
        session
            .insert(SESSION_KEY_2FA_PENDING, user.id)
            .await
            .map_err(|e| AppError::Internal(format!("session: {e}")))?;
        session
            .insert(SESSION_KEY_2FA_NEXT, next.clone())
            .await
            .map_err(|e| AppError::Internal(format!("session: {e}")))?;
        return Ok(axum::response::Redirect::to("/login/2fa").into_response());
    }

    // No 2FA: complete the session now.
    rate_limit::record_success(&state.pool, &ip, &email_norm, now)
        .await
        .map_err(AppError::Db)?;
    mark_authenticated(&session).await;
    auth.login(&user)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    let next = form.next.unwrap_or_else(|| "/".into());
    Ok(axum::response::Redirect::to(&next).into_response())
}

// ─── /login/2fa (GET + POST) ────────────────────────────────────────────

pub async fn login_2fa_page(
    session: Session,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let pending: Option<Uuid> = session
        .get(SESSION_KEY_2FA_PENDING)
        .await
        .map_err(|e| AppError::Internal(format!("session: {e}")))?;
    if pending.is_none() {
        return Ok(axum::response::Redirect::to("/login").into_response());
    }
    let next = params.get("next").cloned().unwrap_or_default();
    Ok(render_response(Login2faPage::new(next)))
}

#[derive(Deserialize)]
pub struct TwoFactorForm {
    pub code: String,
    pub next: Option<String>,
}

pub async fn login_2fa_submit(
    mut auth: AuthSession<Backend>,
    session: Session,
    State(state): State<AppState>,
    axum::Form(form): axum::Form<TwoFactorForm>,
) -> AppResult<Response> {
    let pending: Option<Uuid> = session
        .get(SESSION_KEY_2FA_PENDING)
        .await
        .map_err(|e| AppError::Internal(format!("session: {e}")))?;
    let Some(user_id) = pending else {
        return Ok(axum::response::Redirect::to("/login").into_response());
    };

    // Try TOTP code first; if it fails, try a recovery code.
    let Some((sealed, counter)) = totp::fetch_state(&state.pool, user_id)
        .await
        .map_err(|e| AppError::Internal(format!("totp: {e:?}")))?
    else {
        // 2FA enrollment vanished between request boundaries —
        // back to login.
        session.remove::<Uuid>(SESSION_KEY_2FA_PENDING).await.ok();
        return Ok(axum::response::Redirect::to("/login").into_response());
    };

    let secret = state
        .totp_cipher
        .open(&sealed)
        .map_err(|e| AppError::Internal(format!("cipher: {e:?}")))?;

    let now = OffsetDateTime::now_utc();
    let code_trim = form.code.trim().to_uppercase();
    let matched: Option<(i64, bool)> = match totp::verify_code(&secret, &form.code, counter, now) {
        Ok(step) => Some((step as i64, true)),
        Err(_) => match totp::consume_recovery_code(&state.pool, user_id, &code_trim).await {
            Ok(_) => Some((counter, false)),
            Err(_) => None,
        },
    };

    let Some((matched_step, is_totp)) = matched else {
        let next = form.next.clone().unwrap_or_default();
        return Ok(render_response(
            Login2faPage::new(next).with_error("Invalid code"),
        ));
    };

    // Advance the counter atomically (only for TOTP codes; recovery
    // codes don't touch the counter). This locks out replays of
    // the same TOTP code.
    if is_totp {
        totp::advance_counter(&state.pool, user_id, counter, matched_step)
            .await
            .map_err(|e| AppError::Internal(format!("counter: {e:?}")))?;
    }

    // Fetch the full User record so we can materialise the session.
    let user = sqlx::query_as::<_, crate::auth::User>(
        r#"SELECT id, email, username, display_name, role, hashed_password, is_active, created_at, updated_at, theme, locale
           FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Db)?;

    // Clean the pending-2FA session data.
    session.remove::<Uuid>(SESSION_KEY_2FA_PENDING).await.ok();
    let next = session
        .remove::<String>(SESSION_KEY_2FA_NEXT)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| form.next.clone().unwrap_or_else(|| "/".into()));

    mark_authenticated(&session).await;
    auth.login(&user)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    Ok(axum::response::Redirect::to(&next).into_response())
}

pub async fn register_page() -> AppResult<Response> {
    Ok(render_response(RegisterPage {
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub username: String,
    pub password: String,
    pub password_confirm: String,
}

pub async fn register_submit(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> AppResult<Response> {
    if form.password != form.password_confirm {
        return Ok(render_response(RegisterPage {
            error: "Passwords do not match".into(),
        }));
    }
    if let Err(e) = password::validate_strength(&form.password) {
        // Scrub the password before logging the form body.
        tracing::warn!(
            endpoint = "register",
            error = %e,
            password = password::mask(&form.password),
            "registration rejected"
        );
        return Ok(render_response(RegisterPage {
            error: e.message().into(),
        }));
    }
    create_user(&state.pool, &form.email, &form.username, &form.password)
        .await
        .map_err(|e| match e {
            AppError::Conflict(m) => AppError::Conflict(m),
            other => other,
        })?;
    Ok(axum::response::Redirect::to("/login?next=/ledgers/new").into_response())
}

pub async fn logout(mut auth: AuthSession<Backend>) -> AppResult<axum::response::Redirect> {
    auth.logout()
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    Ok(axum::response::Redirect::to("/login"))
}

// `State` and `Form` re-imports kept here for the helper below.
use axum::{extract::State, Form};
