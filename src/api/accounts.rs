//! Account endpoints for the REST API (`a1-rest-api`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{problem::Problem, ApiUser},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/accounts", get(list).post(create))
        .route("/ledgers/{ledger_id}/accounts/{id}", get(get_one))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AccountDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: Option<String>,
    #[sqlx(rename = "type")]
    pub account_type: String,
    pub subtype: String,
    pub currency: String,
    pub is_archived: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountBody {
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub account_subtype: String,
    pub description: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    // The user must own the ledger.
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let rows: Vec<AccountDto> = sqlx::query_as::<_, AccountDto>(
        "SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
         FROM accounts WHERE ledger_id = $1 ORDER BY type, code NULLS LAST, name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(problem_for_db)?;
    Ok(Json(serde_json::json!({ "data": rows })))
}

async fn get_one(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<AccountDto>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let row: Option<AccountDto> = sqlx::query_as::<_, AccountDto>(
        "SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
         FROM accounts WHERE id = $1 AND ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(problem_for_db)?;
    match row {
        Some(r) => Ok(Json(r)),
        None => Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "account not found",
        )),
    }
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Json(body): Json<CreateAccountBody>,
) -> Result<(StatusCode, Json<AccountDto>), Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    // Look up the ledger currency for the new account.
    let currency: String =
        match sqlx::query_scalar("SELECT base_currency FROM ledgers WHERE id = $1")
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(problem_for_db)?
        {
            Some(c) => c,
            None => {
                return Err(Problem::new(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    "ledger not found",
                ))
            }
        };
    let row: Result<AccountDto, sqlx::Error> = sqlx::query_as::<_, AccountDto>(
        "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency, description)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at",
    )
    .bind(ledger_id)
    .bind(&body.name)
    .bind(body.code.as_deref().filter(|s| !s.is_empty()))
    .bind(&body.account_type)
    .bind(&body.account_subtype)
    .bind(&currency)
    .bind(body.description.as_deref().filter(|s| !s.is_empty()))
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

/// Returns `true` if `ledger_id` exists and is owned by `user_id`.
async fn ledger_owned_by(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    user_id: Uuid,
) -> Result<bool, Problem> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM ledgers WHERE id = $1 AND owner_id = $2")
            .bind(ledger_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(problem_for_db)?;
    Ok(row.is_some())
}

fn problem_for_db(e: sqlx::Error) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        format!("db: {e}"),
    )
}

// Suppress the unused import lint on `Decimal` — we don't
// currently expose balances, but the field set is reserved
// for future expansion.
#[allow(dead_code)]
fn _decimal_unused(_: Decimal) {}
