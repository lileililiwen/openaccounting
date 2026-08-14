use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::Contact,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::contacts::{ContactList, ContactNew},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let contacts = sqlx::query_as::<_, Contact>(
        r#"SELECT id, ledger_id, name, email, phone, kind, created_at, updated_at
           FROM contacts WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(ContactList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        contacts,
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(ContactNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewContactForm {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub kind: String,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewContactForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Ok(render_response(ContactNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name.clone(),
            error: "Name is required".into(),
        }));
    }

    let kind = match form.kind.as_str() {
        "customer" | "vendor" | "both" => form.kind.clone(),
        _ => {
            return Ok(render_response(ContactNew {
                user_id: user.id,
                username: user.username.clone(),
                user_role: user.role.clone(),
                ledger_id,
                ledger_name: ledger.name.clone(),
                error: "Invalid contact kind".into(),
            }));
        }
    };

    let result = sqlx::query(
        r#"INSERT INTO contacts (ledger_id, name, email, phone, kind)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(form.email.as_deref().filter(|s| !s.is_empty()))
    .bind(form.phone.as_deref().filter(|s| !s.is_empty()))
    .bind(&kind)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Contact name already exists in this ledger".into())
        }
        _ => AppError::Db(e),
    })?;

    let contact_id: Uuid = result.get(0);
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "contact",
        Some(contact_id),
        None,
        Some(serde_json::json!({
            "name": name,
            "kind": kind
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/contacts", ledger_id)).into_response())
}
