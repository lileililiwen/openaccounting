//! Automation rule builder (`openapi-sdk`).
//!
//! Owner-only CRUD over `automation_rules`: event → condition →
//! action mappings executed on the existing jobs queue. Editors and
//! other members get 403. The same page manages the per-ledger
//! incoming-events secret used by `POST /api/events/{ledger_id}`.

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
    jobs::automation,
    templates::automations::{AutomationList, AutomationRow},
    AppState,
};

pub async fn page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner_strict(&state, user.id, ledger_id).await?;

    let rows: Vec<AutomationRow> = sqlx::query_as(
        r#"SELECT id, name, trigger, conditions::text AS conditions, action,
                  action_config::text AS action_config, is_enabled
           FROM automation_rules WHERE ledger_id = $1 ORDER BY created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let subscriptions: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, target_url FROM webhook_subscriptions WHERE ledger_id = $1 ORDER BY created_at",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let has_secret: bool = sqlx::query_scalar(
        "SELECT COALESCE(incoming_events_secret, '') <> '' FROM ledgers WHERE id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(crate::templates::render_response(AutomationList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "settings".to_string(),
        rows,
        subscriptions,
        has_secret,
        triggers: automation::allowed_triggers(),
        actions: automation::ACTIONS.to_vec(),
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct CreateRuleForm {
    pub name: String,
    pub trigger: String,
    pub conditions: Option<String>,
    pub action: String,
    pub action_config: Option<String>,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<CreateRuleForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_strict(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    if !automation::allowed_triggers().contains(&form.trigger.as_str()) {
        return Err(AppError::Validation("trigger not on the allowlist".into()));
    }
    if !automation::ACTIONS.contains(&form.action.as_str()) {
        return Err(AppError::Validation("action not on the allowlist".into()));
    }
    let conditions: serde_json::Value = match form.conditions.as_deref().map(str::trim) {
        None | Some("") => serde_json::json!({}),
        Some(raw) => match serde_json::from_str(raw) {
            Ok(v @ serde_json::Value::Object(_)) => v,
            _ => {
                return Err(AppError::Validation(
                    "conditions must be a JSON object".into(),
                ))
            }
        },
    };
    let action_config: serde_json::Value = match form.action_config.as_deref().map(str::trim) {
        None | Some("") => serde_json::json!({}),
        Some(raw) => match serde_json::from_str(raw) {
            Ok(v @ serde_json::Value::Object(_)) => v,
            _ => {
                return Err(AppError::Validation(
                    "action config must be a JSON object".into(),
                ))
            }
        },
    };
    let config_ok = match form.action.as_str() {
        "webhook_post" => action_config.get("subscription_id").is_some(),
        "email_notify" => action_config.get("to").is_some(),
        "categorize_transaction" => action_config.get("cost_center").is_some(),
        _ => false,
    };
    if !config_ok {
        return Err(AppError::Validation(
            "action config missing required key for this action".into(),
        ));
    }

    let rule_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO automation_rules (ledger_id, name, trigger, conditions, action, action_config, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(&form.trigger)
    .bind(&conditions)
    .bind(&form.action)
    .bind(&action_config)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.create",
        "automation_rule",
        Some(rule_id),
        None,
        Some(serde_json::json!({
            "trigger": form.trigger,
            "action": form.action,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/automations")).into_response())
}

pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, rule_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_strict(&state, user.id, ledger_id).await?;
    let updated = sqlx::query(
        "UPDATE automation_rules SET is_enabled = NOT is_enabled, updated_at = now()
         WHERE id = $1 AND ledger_id = $2",
    )
    .bind(rule_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.toggle",
        "automation_rule",
        Some(rule_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/automations")).into_response())
}

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, rule_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_strict(&state, user.id, ledger_id).await?;
    let deleted = sqlx::query("DELETE FROM automation_rules WHERE id = $1 AND ledger_id = $2")
        .bind(rule_id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rule.delete",
        "automation_rule",
        Some(rule_id),
        None,
        None,
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/automations")).into_response())
}

pub async fn rotate_incoming_secret(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_owner_strict(&state, user.id, ledger_id).await?;
    let secret = crate::handlers::events_in::generate_incoming_secret();
    sqlx::query("UPDATE ledgers SET incoming_events_secret = $2 WHERE id = $1")
        .bind(ledger_id)
        .bind(&secret)
        .execute(&state.pool)
        .await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "rotate_incoming_secret",
        "ledger",
        Some(ledger_id),
        None,
        None,
    )
    .await;
    let _ = secret;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/automations")).into_response())
}
