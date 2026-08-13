use axum::{extract::State, response::Response, Form};
use axum_login::AuthSession;
use serde::Deserialize;

use crate::{
    auth::{change_password as auth_change_password, Backend, User},
    error::{AppError, AppResult},
    templates::{
        account::{AccountPage, PasswordForm},
        render_response,
    },
    AppState,
};

#[derive(Deserialize)]
pub struct PasswordFormInput {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

pub async fn show(auth: AuthSession<Backend>) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();
    Ok(render_response(AccountPage::new(user)))
}

pub async fn change_password(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<PasswordFormInput>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?.clone();

    // 1. Cross-field check first (cheapest, most precise).
    if form.new_password != form.confirm_password {
        return Ok(render_response(AccountPage::with_error(
            user,
            "New password and confirmation do not match.",
            PasswordForm::from_post(
                form.current_password,
                form.new_password,
                form.confirm_password,
            ),
        )));
    }

    // 2. Delegate to the auth helper for current-password verification
    //    and new-password strength + persist.
    match auth_change_password(
        &state.pool,
        user.id,
        &form.current_password,
        &form.new_password,
    )
    .await
    {
        Ok(()) => Ok(render_response(AccountPage::with_flash(
            user,
            "Password updated.",
        ))),
        Err(AppError::Unauthorized) => Ok(render_response(AccountPage::with_error(
            user,
            "Current password is incorrect.",
            PasswordForm::from_post(
                form.current_password,
                form.new_password,
                form.confirm_password,
            ),
        ))),
        Err(AppError::Validation(m)) => Ok(render_response(AccountPage::with_error(
            user,
            m,
            PasswordForm::from_post(
                form.current_password,
                form.new_password,
                form.confirm_password,
            ),
        ))),
        Err(e) => Err(e),
    }
}

#[allow(dead_code)]
fn _user_marker(_u: &User) {}
