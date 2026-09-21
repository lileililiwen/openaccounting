//! Cost centers and projects (`accounting-dimensions`).
//!
//! Ledger-scoped analysis dimensions. Postings reference them
//! optionally; reports slice by them with an Unassigned bucket for
//! untagged postings.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::dimensions::{DimensionList, DimensionRow},
    AppState,
};

#[derive(Deserialize)]
pub struct NewDimensionForm {
    pub name: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let cost_centers = sqlx::query_as::<_, DimensionRow>(
        "SELECT id, name FROM cost_centers WHERE ledger_id = $1 ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let projects = sqlx::query_as::<_, DimensionRow>(
        "SELECT id, name FROM projects WHERE ledger_id = $1 ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(crate::templates::render_response(DimensionList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "reports".to_string(),
        cost_centers,
        projects,
    }))
}

async fn create_named(
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    table: &str,
    entity: &str,
    name: &str,
) -> AppResult<Uuid> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    // Table is an internal constant, never user input.
    let sql = format!("INSERT INTO {table} (ledger_id, name) VALUES ($1, $2) RETURNING id");
    let id: Uuid = sqlx::query_scalar(&sql)
        .bind(ledger_id)
        .bind(name)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e {
                if db.code().as_deref() == Some("23505") {
                    return AppError::Conflict(format!("{entity} '{name}' already exists"));
                }
            }
            AppError::Db(e)
        })?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "create",
        entity,
        Some(id),
        None,
        Some(serde_json::json!({ "name": name })),
    )
    .await;
    Ok(id)
}

pub async fn create_cost_center(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewDimensionForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    create_named(
        &state,
        ledger_id,
        user.id,
        "cost_centers",
        "cost_center",
        &form.name,
    )
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/dimensions")).into_response())
}

pub async fn create_project(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewDimensionForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    create_named(
        &state, ledger_id, user.id, "projects", "project", &form.name,
    )
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/dimensions")).into_response())
}

pub async fn delete_cost_center(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    // Postings referencing it fall back to Unassigned (ON DELETE SET NULL).
    sqlx::query("DELETE FROM cost_centers WHERE id = $1 AND ledger_id = $2")
        .bind(id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/dimensions")).into_response())
}

pub async fn delete_project(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query("DELETE FROM projects WHERE id = $1 AND ledger_id = $2")
        .bind(id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/dimensions")).into_response())
}
