//! POST /account/locale — set the user's preferred locale.
//!
//! Validates the value against the day-1 set and redirects
//! back to the referring page.

use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    AppState,
};

#[derive(Deserialize)]
pub struct LocaleForm {
    pub locale: String,
    #[serde(default)]
    pub next: Option<String>,
}

pub async fn set_locale(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<LocaleForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    // Reject anything outside the day-1 set so the column
    // CHECK constraint isn't the only thing standing between
    // us and a `Cow::Owned` of an attacker-controlled string.
    let allowed = ["en", "zh-CN", "es", "fr", "de", "ja"];
    if !allowed.contains(&form.locale.as_str()) {
        return Err(AppError::Validation(format!(
            "unsupported locale: {}",
            form.locale
        )));
    }

    sqlx::query("UPDATE users SET locale = $1, updated_at = now() WHERE id = $2")
        .bind(&form.locale)
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    let next = form
        .next
        .as_deref()
        .filter(|s| s.starts_with('/') && !s.starts_with("//"))
        .unwrap_or("/account");

    Ok(Redirect::to(next).into_response())
}
