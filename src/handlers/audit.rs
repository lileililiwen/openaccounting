use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::Response;
use axum_login::AuthSession;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::audit::AuditLogPage,
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let limit: i64 = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(50);
    let offset: i64 = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let action_filter = q.get("action").map(|s| s.as_str());
    let entity_filter = q.get("entity_type").map(|s| s.as_str());

    let entries = audit::list(
        &state.pool,
        Some(ledger_id),
        None,
        action_filter,
        entity_filter,
        None,
        None,
        limit,
        offset,
    )
    .await?;

    // Get actor names for the entries
    let mut entries_with_names = Vec::new();
    for entry in entries {
        let actor_name: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(entry.actor_id)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
        entries_with_names.push((entry, actor_name.unwrap_or_default()));
    }

    Ok(render_response(AuditLogPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: _ledger.name,
        entries: entries_with_names,
        offset,
        limit,
    }))
}
