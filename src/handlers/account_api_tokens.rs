//! HTML page for managing per-user API tokens
//! (`a1-rest-api`).
//!
//! GET  `/account/api-tokens`            — list + form to issue.
//! POST `/account/api-tokens/create`     — issue; shows plaintext once.
//! POST `/account/api-tokens/{id}/revoke`— mark revoked.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{api_token, Backend},
    error::{AppError, AppResult},
    templates::{
        api_tokens::{ApiTokenListPage, ApiTokenShowPage, ApiTokenSummary},
        render_response,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/api-tokens", get(page))
        .route("/account/api-tokens/create", post(create))
        .route("/account/api-tokens/{id}/revoke", post(revoke))
}

#[derive(Debug, Deserialize)]
pub struct CreateForm {
    pub name: String,
}

async fn page(auth: AuthSession<Backend>, State(state): State<AppState>) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let tokens: Vec<ApiTokenSummary> = api_token::list_tokens(&state.pool, user.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(render_response(ApiTokenListPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        tokens,
        plaintext: None,
    }))
}

async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<CreateForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if form.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    let issued = api_token::issue_token(&state.pool, user.id, &form.name).await?;
    let tokens: Vec<ApiTokenSummary> = api_token::list_tokens(&state.pool, user.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(render_response(ApiTokenShowPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        tokens,
        plaintext: Some(issued.plaintext),
        plaintext_name: Some(form.name),
    })
    .into_response())
}

async fn revoke(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = api_token::revoke_token(&state.pool, id, user.id).await?;
    Ok(Redirect::to("/account/api-tokens").into_response())
}
