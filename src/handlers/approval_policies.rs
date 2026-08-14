//! HTTP handlers for reimbursement approval policies:
//! list, new, create, delete.

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::approval_policies::{PolicyList, PolicyNew, PolicyRow},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let rows: Vec<PolicyRow> = sqlx::query_as(
        r#"SELECT id, name, min_amount, approver_role, level
           FROM reimbursement_approval_policies
           WHERE ledger_id = $1
           ORDER BY min_amount, level"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(PolicyList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        policies: rows,
        error: String::new(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(PolicyNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewPolicyForm {
    pub name: String,
    pub min_amount: Decimal,
    pub approver_role: String,
    pub level: i32,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewPolicyForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    if form.name.trim().is_empty() {
        return Ok(render_response(PolicyNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "name is required".into(),
        })
        .into_response());
    }
    if form.level < 1 {
        return Ok(render_response(PolicyNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "level must be >= 1".into(),
        })
        .into_response());
    }
    if form.min_amount < Decimal::ZERO {
        return Ok(render_response(PolicyNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "min_amount must be >= 0".into(),
        })
        .into_response());
    }
    if form.approver_role != "Admin" && form.approver_role != "Accountant" {
        return Ok(render_response(PolicyNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "approver_role must be Admin or Accountant".into(),
        })
        .into_response());
    }
    sqlx::query(
        r#"INSERT INTO reimbursement_approval_policies
              (ledger_id, name, min_amount, approver_role, level)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(ledger_id)
    .bind(form.name.trim())
    .bind(form.min_amount)
    .bind(&form.approver_role)
    .bind(form.level)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "approval_policy.create",
        "reimbursement_approval_policy",
        None,
        None,
        Some(serde_json::json!({
            "name": form.name,
            "min_amount": form.min_amount,
            "approver_role": form.approver_role,
            "level": form.level
        })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/approval-policies")).into_response())
}

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, policy_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    sqlx::query(
        r#"DELETE FROM reimbursement_approval_policies
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(policy_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "approval_policy.delete",
        "reimbursement_approval_policy",
        Some(policy_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/approval-policies")).into_response())
}
