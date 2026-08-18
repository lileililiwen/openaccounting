//! Ledger endpoints for the REST API (`a1-rest-api`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{problem::Problem, ApiUser},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers", get(list).post(create))
        .route("/ledgers/{id}", get(get_one))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LedgerDto {
    pub id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub timezone: String,
    pub basis: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLedgerBody {
    pub name: String,
    pub base_currency: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_basis")]
    pub basis: String,
}

fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_basis() -> String {
    "accrual".to_string()
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
) -> Result<Json<serde_json::Value>, Problem> {
    let rows: Vec<LedgerDto> = sqlx::query_as::<_, LedgerDto>(
        "SELECT id, name, base_currency, timezone, basis, created_at, updated_at
         FROM ledgers WHERE owner_id = $1
         ORDER BY created_at DESC",
    )
    .bind(user.0)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;
    Ok(Json(serde_json::json!({ "data": rows })))
}

async fn get_one(
    State(state): State<AppState>,
    user: ApiUser,
    Path(id): Path<Uuid>,
) -> Result<Json<LedgerDto>, Problem> {
    let row: Option<LedgerDto> = sqlx::query_as::<_, LedgerDto>(
        "SELECT id, name, base_currency, timezone, basis, created_at, updated_at
         FROM ledgers WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(user.0)
    .fetch_optional(&state.pool)
    .await
    .map_err(problem_for_db)?;
    match row {
        Some(r) => Ok(Json(r)),
        None => Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        )),
    }
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Json(body): Json<CreateLedgerBody>,
) -> Result<(StatusCode, Json<LedgerDto>), Problem> {
    let row: Result<LedgerDto, sqlx::Error> = sqlx::query_as::<_, LedgerDto>(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, base_currency, timezone, basis, created_at, updated_at",
    )
    .bind(user.0)
    .bind(&body.name)
    .bind(&body.base_currency)
    .bind(&body.timezone)
    .bind(&body.basis)
    .fetch_one(&state.pool)
    .await;
    match row {
        Ok(r) => Ok((StatusCode::CREATED, Json(r))),
        Err(e) => Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            format!("insert failed: {e}"),
        )),
    }
}

fn problem_for_db(e: sqlx::Error) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        format!("db: {e}"),
    )
}
