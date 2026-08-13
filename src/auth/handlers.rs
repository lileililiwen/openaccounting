use crate::templates::render_response;
use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::{extract::State, response::Redirect, Form, Router};
use axum_login::AuthSession;
use serde::Deserialize;

use crate::{
    auth::{create_user, Backend, Credentials},
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

pub async fn login_submit(
    mut auth: AuthSession<Backend>,
    State(_state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
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
            return Ok(render_response(LoginPage {
                error: "Invalid email or password".into(),
                next: form.next.clone().unwrap_or_default(),
            }));
        }
    };
    auth.login(&user)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    let next = form.next.unwrap_or_else(|| "/".into());
    Ok(Redirect::to(&next).into_response())
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
    Ok(Redirect::to("/login?next=/ledgers/new").into_response())
}

pub async fn logout(mut auth: AuthSession<Backend>) -> AppResult<Redirect> {
    auth.logout()
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;
    Ok(Redirect::to("/login"))
}
