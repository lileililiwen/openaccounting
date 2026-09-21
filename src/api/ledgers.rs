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
    api::{
        helpers::{fingerprint, idempotency_lookup, idempotency_store},
        problem::Problem,
        ApiTokenId, ApiUser,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers", get(list).post(create))
        .route("/ledgers/{id}", get(get_one).patch(update))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LedgerDto {
    pub id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub timezone: String,
    pub basis: String,
    pub append_only: bool,
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
        "SELECT id, name, base_currency, timezone, basis, append_only, created_at, updated_at
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
        "SELECT id, name, base_currency, timezone, basis, append_only, created_at, updated_at
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
    token: ApiTokenId,
    headers: axum::http::HeaderMap,
    Json(raw): Json<serde_json::Value>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, axum::http::HeaderValue); 1],
        String,
    ),
    Problem,
> {
    // Idempotency (`openapi-sdk`): replay the stored response on key
    // match within the 24 h window.
    let idem_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let fp = fingerprint(&raw);
    if let Some(k) = &idem_key {
        if let Some((status, resp_body)) = idempotency_lookup(&state.pool, k, token.0, &fp).await? {
            return Ok((
                StatusCode::from_u16(status).unwrap_or(StatusCode::CREATED),
                [(
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderValue::from_static("application/json"),
                )],
                resp_body,
            ));
        }
    }

    let body: CreateLedgerBody = serde_json::from_value(raw)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    let row: Result<LedgerDto, sqlx::Error> = sqlx::query_as::<_, LedgerDto>(
        "INSERT INTO ledgers (owner_id, name, base_currency, timezone, basis)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, base_currency, timezone, basis, append_only, created_at, updated_at",
    )
    .bind(user.0)
    .bind(&body.name)
    .bind(&body.base_currency)
    .bind(&body.timezone)
    .bind(&body.basis)
    .fetch_one(&state.pool)
    .await;
    let dto = match row {
        Ok(r) => r,
        Err(e) => {
            return Err(Problem::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("insert failed: {e}"),
            ))
        }
    };
    let resp = serde_json::to_string(&dto).map_err(|e| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            e.to_string(),
        )
    })?;
    if let Some(k) = idem_key {
        idempotency_store(&state.pool, &k, token.0, &fp, 201, &resp).await?;
    }
    Ok((
        StatusCode::CREATED,
        [(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        )],
        resp,
    ))
}

#[derive(Debug, Deserialize)]
pub struct UpdateLedgerBody {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub basis: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    user: ApiUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateLedgerBody>,
) -> Result<Json<LedgerDto>, Problem> {
    if body
        .basis
        .as_deref()
        .is_some_and(|b| b != "accrual" && b != "cash")
    {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "basis must be 'accrual' or 'cash'",
        ));
    }
    let row: Option<LedgerDto> = sqlx::query_as::<_, LedgerDto>(
        "UPDATE ledgers
         SET name = COALESCE($2, name),
             timezone = COALESCE($3, timezone),
             basis = COALESCE($4, basis),
             updated_at = now()
         WHERE id = $1 AND owner_id = $5
         RETURNING id, name, base_currency, timezone, basis, append_only, created_at, updated_at",
    )
    .bind(id)
    .bind(body.name.as_deref().filter(|s| !s.trim().is_empty()))
    .bind(body.timezone.as_deref().filter(|s| !s.trim().is_empty()))
    .bind(body.basis.as_deref())
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

fn problem_for_db(e: sqlx::Error) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        format!("db: {e}"),
    )
}
