//! Per-user theme preference (`u8-dark-mode`).
//!
//! Three values are accepted: `"system"` (default — follow
//! the OS `prefers-color-scheme`), `"light"`, or `"dark"`.
//! Invalid values are rejected with 400. The preference is
//! persisted on the `users.theme` column added by migration
//! 0031.

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
pub struct ThemeForm {
    pub theme: String,
    /// Where to send the user after the toggle. Defaults to
    /// `/account` so the call from any page stays usable.
    #[serde(default)]
    pub next: Option<String>,
}

/// POST /account/theme — flip the user's preference and
/// redirect back to `next` (or `/account`).
pub async fn set_theme(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<ThemeForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let theme = match form.theme.as_str() {
        "system" | "light" | "dark" => form.theme,
        other => {
            return Err(AppError::Validation(format!("Invalid theme: {other}")));
        }
    };

    sqlx::query("UPDATE users SET theme = $1, updated_at = now() WHERE id = $2")
        .bind(&theme)
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
