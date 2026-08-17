use crate::templates::render_response;
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

use crate::{
    auth::{
        create_user,
        rate_limit::{self, Decision},
        Backend, Credentials,
    },
    error::{AppError, AppResult},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", axum::routing::get(login_page).post(login_submit))
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
    Ok(render_response(LoginPage {
        error: String::new(),
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

    // Successful auth: audit row + reset per-account failure counter.
    rate_limit::record_success(&state.pool, &ip, &email_norm, now)
        .await
        .map_err(AppError::Db)?;

    auth.login(&user)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    let next = form.next.unwrap_or_else(|| "/".into());
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
    if form.password.len() < 8 {
        return Ok(render_response(RegisterPage {
            error: "Password must be at least 8 characters".into(),
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
