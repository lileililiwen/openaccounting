//! Admin endpoint that verifies the audit hash-chain
//! (`d1-audit-chain`).

use axum::extract::State;
use axum_login::AuthSession;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    templates::render_response,
    AppState,
};

/// GET /admin/audit/verify — walk the chain and report break points.
pub async fn verify_chain(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<axum::response::Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let breaks = crate::audit::chain::verify(&state.pool).await?;
    let tail = crate::audit::chain::latest_hash_hex(&state.pool).await?;
    let (entries_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_entries")
        .fetch_one(&state.pool)
        .await?;

    let page = crate::templates::admin::AuditVerifyPage {
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: uuid::Uuid::nil(),
        ledger_name: String::new(),
        intact: breaks.is_empty(),
        entries_count,
        tail_hash: tail.unwrap_or_default(),
        breakpoints: breaks
            .into_iter()
            .map(|b| crate::templates::admin::BreakPointRow {
                id: b.id,
                reason: b.reason,
            })
            .collect(),
    };
    Ok(render_response(page))
}
