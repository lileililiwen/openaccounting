//! Budget endpoints (`api-v2-coverage`).

use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{
        helpers::{db_problem, next_link, page_from, require_access, with_next_link, PageParams},
        problem::Problem,
        ApiUser,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/budgets", get(list).post(create))
        .route("/ledgers/{ledger_id}/budgets/{id}", delete(remove))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BudgetDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub account_id: Uuid,
    pub account_name: String,
    pub period: String,
    pub amount: Decimal,
    pub alert_threshold: Decimal,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudgetBody {
    pub account_id: Uuid,
    pub period: String,
    pub amount: Decimal,
    pub alert_threshold: Option<Decimal>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let page = page_from(&params)?;
    let rows: Vec<BudgetDto> = sqlx::query_as(
        r#"SELECT b.id, b.ledger_id, b.account_id, a.name AS account_name,
                  b.period, b.amount, b.alert_threshold, b.start_date, b.end_date
           FROM budgets b JOIN accounts a ON a.id = b.account_id
           WHERE b.ledger_id = $1
           ORDER BY b.start_date LIMIT $2 OFFSET $3"#,
    )
    .bind(ledger_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;

    // vs-actual per budget (period-to-date expense totals).
    let mut out = Vec::with_capacity(rows.len());
    for b in &rows {
        let actual: (Decimal,) = sqlx::query_as(
            r#"SELECT COALESCE(SUM(p.amount), 0)
               FROM postings p
               JOIN transactions t ON t.id = p.transaction_id
               WHERE p.account_id = $1 AND t.kind <> 'draft'
                     AND t.txn_date BETWEEN $2 AND $3"#,
        )
        .bind(b.account_id)
        .bind(b.start_date)
        .bind(b.end_date)
        .fetch_one(&state.pool)
        .await
        .map_err(db_problem)?;
        out.push(serde_json::json!({
            "id": b.id,
            "account": b.account_name,
            "period": b.period,
            "amount": b.amount,
            "actual": actual.0,
            "pct_used": if b.amount.is_zero() { None } else {
                Some((actual.0 / b.amount).round_dp(4))
            },
            "start_date": b.start_date,
            "end_date": b.end_date,
        }));
    }
    let link = next_link(
        &format!("/api/v1/ledgers/{ledger_id}/budgets"),
        page,
        rows.len(),
    );
    Ok(with_next_link(
        Json(serde_json::json!({ "data": out })).into_response(),
        link,
    ))
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Json(raw): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let body: CreateBudgetBody = serde_json::from_value(raw)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    if !matches!(body.period.as_str(), "monthly" | "quarterly" | "yearly") {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "period must be monthly, quarterly, or yearly",
        ));
    }
    if body.amount <= Decimal::ZERO {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "amount must be positive",
        ));
    }
    let threshold = body.alert_threshold.unwrap_or(Decimal::new(8, 1)); // 0.8
    let account_ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE id = $1 AND ledger_id = $2")
            .bind(body.account_id)
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_problem)?;
    if account_ok.is_none() {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "account not found in this ledger",
        ));
    }
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO budgets (ledger_id, account_id, period, amount, alert_threshold, start_date, end_date)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(ledger_id)
    .bind(body.account_id)
    .bind(&body.period)
    .bind(body.amount)
    .bind(threshold)
    .bind(body.start_date)
    .bind(body.end_date)
    .fetch_one(&state.pool)
    .await
    .map_err(db_problem)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

async fn remove(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let deleted = sqlx::query("DELETE FROM budgets WHERE id = $1 AND ledger_id = $2")
        .bind(id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await
        .map_err(db_problem)?;
    if deleted.rows_affected() == 0 {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "budget not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
