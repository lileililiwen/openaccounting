//! HTTP handlers for the reconciliation rules engine:
//! list, new, create, toggle, delete.

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::reconciliation_rules::{Rule, RuleKind},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::rules::{RuleList, RuleNew, RuleRow},
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let rows: Vec<RuleRow> = sqlx::query_as(
        r#"SELECT id, name, kind, priority, is_active
           FROM reconciliation_rules
           WHERE ledger_id = $1
           ORDER BY priority, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(RuleList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: _ledger.name,
        rules: rows,
        error: String::new(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    Ok(render_response(RuleNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewRuleForm {
    pub name: String,
    pub kind: String,
    pub priority: i32,
    /// JSON-encoded predicate. The form template asks the
    /// user to paste `{"payee_glob":"STARBUCKS%"}` (a LIKE
    /// pattern). The handler validates that the value parses
    /// as JSON before persisting.
    pub predicate: String,
    /// JSON-encoded action, e.g.
    /// `{"gl_account_id":"<uuid>"}`.
    pub action: String,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewRuleForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let kind = RuleKind::parse(&form.kind)
        .ok_or_else(|| AppError::Validation(format!("unknown kind '{}'", form.kind)))?;
    if form.name.trim().is_empty() {
        return Ok(render_rule_new_with_error(
            state,
            user.id,
            user.username.clone(),
            user.role.clone(),
            ledger_id,
            _ledger.name,
            "name is required".into(),
        )
        .await);
    }
    let predicate: serde_json::Value = serde_json::from_str(&form.predicate)
        .map_err(|e| AppError::Validation(format!("predicate is not valid JSON: {e}")))?;
    let action: serde_json::Value = serde_json::from_str(&form.action)
        .map_err(|e| AppError::Validation(format!("action is not valid JSON: {e}")))?;
    sqlx::query(
        r#"INSERT INTO reconciliation_rules
              (ledger_id, name, kind, priority, predicate, action, is_active)
           VALUES ($1, $2, $3, $4, $5, $6, TRUE)"#,
    )
    .bind(ledger_id)
    .bind(form.name.trim())
    .bind(kind.as_str())
    .bind(form.priority)
    .bind(&predicate)
    .bind(&action)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.create",
        "reconciliation_rule",
        None,
        None,
        Some(serde_json::json!({"name": form.name, "kind": kind.as_str()})),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/rules")).into_response())
}

pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, rule_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query(
        r#"UPDATE reconciliation_rules
           SET is_active = NOT is_active
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(rule_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.toggle",
        "reconciliation_rule",
        Some(rule_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/rules")).into_response())
}

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, rule_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query(
        r#"DELETE FROM reconciliation_rules
           WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(rule_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.delete",
        "reconciliation_rule",
        Some(rule_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/rules")).into_response())
}

// Avoid pulling another small struct inline.
async fn render_rule_new_with_error(
    _state: AppState,
    user_id: Uuid,
    username: String,
    user_role: String,
    ledger_id: Uuid,
    ledger_name: String,
    error: String,
) -> Response {
    render_response(RuleNew {
        user_id,
        username,
        user_role,
        ledger_id,
        ledger_name,
        error,
    })
    .into_response()
}

/// Suppress the unused-helper warning. The helper exists for
/// the "name is required" re-render branch in `create` but
/// the compiler doesn't see it as used because the simpler
/// path is in use.
#[allow(dead_code)]
fn _unused(_: &Rule) {}
