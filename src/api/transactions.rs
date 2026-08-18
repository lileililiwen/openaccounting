//! Transaction endpoints for the REST API (`a1-rest-api`).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{problem::Problem, ApiUser},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/transactions", get(list).post(create))
        .route("/ledgers/{ledger_id}/transactions/{id}", get(get_one))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TransactionDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub currency: String,
    pub kind: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PostingBody {
    pub account_id: Uuid,
    pub direction: String, // "DEBIT" | "CREDIT"
    pub amount: Decimal,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionBody {
    pub date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub kind: Option<String>,
    pub lines: Vec<PostingBody>,
}

/// Single-create response.
#[derive(Debug, Serialize)]
pub struct CreateTransactionResponse {
    pub id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub total: Decimal,
}

/// `Idempotency-Key` cache. We store the raw JSON response and
/// replay it on duplicate POSTs. The cache is per-process; a
/// restart loses it — fine for v1.
#[derive(Debug, Clone)]
struct IdempotencyRecord {
    status: u16,
    body: String,
}

static IDEMPOTENCY_CACHE: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<String, IdempotencyRecord>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let rows: Vec<TransactionDto> = sqlx::query_as::<_, TransactionDto>(
        "SELECT id, ledger_id, txn_date, description, payee, reference, currency, kind, created_at, updated_at
         FROM transactions WHERE ledger_id = $1
         ORDER BY txn_date DESC, created_at DESC
         LIMIT 100",
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
) -> Result<Json<TransactionDto>, Problem> {
    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    let row: Option<TransactionDto> = sqlx::query_as::<_, TransactionDto>(
        "SELECT id, ledger_id, txn_date, description, payee, reference, currency, kind, created_at, updated_at
         FROM transactions WHERE id = $1 AND ledger_id = $2",
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
            "transaction not found",
        )),
    }
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateTransactionBody>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, axum::http::HeaderValue); 1],
        String,
    ),
    Problem,
> {
    // Idempotency-Key: replay the cached response when present.
    let idem_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if let Some(k) = &idem_key {
        if let Some(rec) = IDEMPOTENCY_CACHE.lock().unwrap().get(k).cloned() {
            let status = StatusCode::from_u16(rec.status).unwrap_or(StatusCode::CREATED);
            return Ok((
                status,
                [(
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderValue::from_static("application/json"),
                )],
                rec.body,
            ));
        }
    }

    if !ledger_owned_by(&state.pool, ledger_id, user.0).await? {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    }
    if body.lines.len() < 2 {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Postings do not balance (need at least two postings)",
        ));
    }
    let debits: Decimal = body
        .lines
        .iter()
        .filter(|p| p.direction.eq_ignore_ascii_case("DEBIT"))
        .map(|p| p.amount)
        .sum();
    let credits: Decimal = body
        .lines
        .iter()
        .filter(|p| p.direction.eq_ignore_ascii_case("CREDIT"))
        .map(|p| p.amount)
        .sum();
    if debits != credits {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Postings do not balance",
        ));
    }
    // Verify every account exists in this ledger.
    for l in &body.lines {
        let exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE id = $1 AND ledger_id = $2")
                .bind(l.account_id)
                .bind(ledger_id)
                .fetch_optional(&state.pool)
                .await
                .map_err(problem_for_db)?;
        if exists.is_none() {
            return Err(Problem::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("unknown account {}", l.account_id),
            ));
        }
    }
    let currency: String =
        match sqlx::query_scalar("SELECT base_currency FROM ledgers WHERE id = $1")
            .bind(ledger_id)
            .fetch_one(&state.pool)
            .await
        {
            Ok(c) => c,
            Err(_) => {
                return Err(Problem::new(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    "ledger not found",
                ))
            }
        };
    let kind = body.kind.clone().unwrap_or_else(|| "standard".to_string());
    let mut tx = state.pool.begin().await.map_err(problem_for_db)?;
    let txn_id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, currency, kind, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(body.date)
    .bind(&body.description)
    .bind(body.payee.as_deref())
    .bind(body.reference.as_deref())
    .bind(&currency)
    .bind(&kind)
    .bind(user.0)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", format!("insert txn: {e}")))?;
    for l in &body.lines {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(txn_id)
        .bind(l.account_id)
        .bind(l.amount)
        .bind(&l.direction)
        .bind(l.memo.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("posting: {e}"),
            )
        })?;
    }
    tx.commit().await.map_err(problem_for_db)?;

    let total = debits;
    let resp = CreateTransactionResponse {
        id: txn_id,
        date: body.date,
        description: body.description.clone(),
        total,
    };
    let body_str = serde_json::to_string(&resp).unwrap_or_default();

    if let Some(k) = idem_key {
        IDEMPOTENCY_CACHE.lock().unwrap().insert(
            k,
            IdempotencyRecord {
                status: StatusCode::CREATED.as_u16(),
                body: body_str.clone(),
            },
        );
    }

    Ok((
        StatusCode::CREATED,
        [(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        )],
        body_str,
    ))
}

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
