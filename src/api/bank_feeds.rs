//! Read-only bank-feed link endpoints (`api-v2-coverage`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    api::{helpers::require_access, problem::Problem, ApiUser},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/bank-feed-links", get(list))
        .route("/ledgers/{ledger_id}/bank-feed-links/{id}", get(get_one))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BankFeedLinkDto {
    pub id: Uuid,
    pub provider: String,
    pub institution_id: Option<String>,
    pub account_id_at_provider: Option<String>,
    pub status: String,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let rows: Vec<BankFeedLinkDto> = sqlx::query_as(
        "SELECT id, provider, institution_id, account_id_at_provider, status, last_synced_at
         FROM bank_feed_links WHERE ledger_id = $1 ORDER BY created_at",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(crate::api::helpers::db_problem)?;
    Ok(Json(serde_json::json!({ "data": rows })))
}

async fn get_one(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<BankFeedLinkDto>, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let row: Option<BankFeedLinkDto> = sqlx::query_as(
        "SELECT id, provider, institution_id, account_id_at_provider, status, last_synced_at
         FROM bank_feed_links WHERE id = $1 AND ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(crate::api::helpers::db_problem)?;
    row.map(Json).ok_or_else(|| {
        Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "bank feed link not found",
        )
    })
}
