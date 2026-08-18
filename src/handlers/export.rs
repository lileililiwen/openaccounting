//! HTTP handlers for full-ledger export (`o1-ledger-export`).
//!
//! Routes:
//! * `GET /ledgers/{id}/export.json`
//! * `GET /ledgers/{id}/export.beancount`
//! * `GET /account/export-all.json`
//!
//! All three require owner or editor on the target ledger;
//! viewers receive 403.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use axum_login::AuthSession;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    export,
    handlers::ledgers,
    AppState,
};

/// GET /ledgers/{id}/export.json
pub async fn export_json(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let snapshot = load_ledger_snapshot(&state, user.id, ledger_id).await?;
    let body = export::json::to_string_pretty(&snapshot);
    Ok(json_attachment(body, &format!("ledger-{ledger_id}.json")))
}

/// GET /ledgers/{id}/export.beancount
pub async fn export_beancount(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let snapshot = load_ledger_snapshot(&state, user.id, ledger_id).await?;
    let body = export::beancount::render(&snapshot);
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (
                header::CONTENT_DISPOSITION,
                axum::http::HeaderValue::from_str(&format!(
                    "attachment; filename=\"ledger-{ledger_id}.beancount\""
                ))
                .map_err(|e| AppError::Internal(e.to_string()))?,
            ),
        ],
        body,
    )
        .into_response())
}

/// GET /account/export-all.json
///
/// Bundles every ledger the caller owns OR is an editor on, in a
/// single JSON document. The owner-only check is enforced per
/// ledger via [`ensure_owner_or_editor`].
pub async fn export_all_json(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let mut snapshots = Vec::new();
    // Owned ledgers.
    let owned: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM ledgers WHERE owner_id = $1 ORDER BY created_at")
            .bind(user.id)
            .fetch_all(&state.pool)
            .await?;
    for id in owned {
        snapshots.push(export::LedgerSnapshot::load(&state.pool, id).await?);
    }
    // Editor-shared ledgers (owner rows are not in this table).
    let shared: Vec<Uuid> = sqlx::query_scalar(
        "SELECT ledger_id FROM ledger_members WHERE user_id = $1 AND role = 'editor' ORDER BY ledger_id",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    for id in shared {
        let already = snapshots.iter().any(|s| s.ledger.id == id);
        if !already {
            snapshots.push(export::LedgerSnapshot::load(&state.pool, id).await?);
        }
    }

    #[derive(Serialize)]
    struct AllEnvelope<'a> {
        format: &'static str,
        format_version: u32,
        exported_at: chrono::DateTime<chrono::Utc>,
        ledgers: Vec<&'a export::LedgerSnapshot>,
    }

    let envelope = AllEnvelope {
        format: export::json::FORMAT,
        format_version: export::json::FORMAT_VERSION,
        exported_at: chrono::Utc::now(),
        ledgers: snapshots.iter().collect(),
    };
    let body = serde_json::to_string_pretty(&envelope)
        .map_err(|e| AppError::Internal(format!("export_all_json: {e}")))?;
    Ok(json_attachment(body, "export-all.json"))
}

async fn load_ledger_snapshot(
    state: &AppState,
    user_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<export::LedgerSnapshot> {
    // Authorization: owner or editor. Viewers must NOT receive
    // the full chart of accounts + transaction history.
    let (role, _) = ensure_owner_or_editor(state, user_id, ledger_id).await?;
    tracing::debug!(role = %role, "export authorized");
    Ok(export::LedgerSnapshot::load(&state.pool, ledger_id).await?)
}

/// Verify the caller is the ledger's owner OR has the `editor`
/// role on it. Viewers (and unrelated users) are rejected with
/// 403 — the spec requires viewer-403.
pub async fn ensure_owner_or_editor(
    state: &AppState,
    user_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<(String, Uuid)> {
    let (ledger, role) = ledgers::ensure_access(state, user_id, ledger_id).await?;
    if role != "owner" && role != "editor" {
        return Err(AppError::Forbidden);
    }
    Ok((role, ledger.id))
}

fn json_attachment(body: String, filename: &str) -> Response {
    let disp = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| axum::http::HeaderValue::from_static("attachment"));
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/json; charset=utf-8"),
            ),
            (header::CONTENT_DISPOSITION, disp),
        ],
        body,
    )
        .into_response()
}
