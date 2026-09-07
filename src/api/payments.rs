//! Payment endpoints (`api-v2-coverage`).

use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
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
        .route("/ledgers/{ledger_id}/payments", get(list).post(create))
        .route("/ledgers/{ledger_id}/payments/{id}", get(get_one))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PaymentDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub contact_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub transaction_id: Option<Uuid>,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub payment_method: String,
    pub reference: Option<String>,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePaymentBody {
    pub contact_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub payment_method: String,
    pub reference: Option<String>,
    /// "received" (AR) or "made" (AP).
    pub kind: String,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let page = page_from(&params)?;
    let rows: Vec<PaymentDto> = sqlx::query_as(
        "SELECT id, ledger_id, contact_id, invoice_id, transaction_id, amount,
                payment_date, payment_method, reference, kind
         FROM payments WHERE ledger_id = $1
         ORDER BY payment_date DESC, created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(ledger_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;
    let link = next_link(
        &format!("/api/v1/ledgers/{ledger_id}/payments"),
        page,
        rows.len(),
    );
    Ok(with_next_link(
        Json(serde_json::json!({ "data": rows })).into_response(),
        link,
    ))
}

async fn get_one(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PaymentDto>, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let row: Option<PaymentDto> = sqlx::query_as(
        "SELECT id, ledger_id, contact_id, invoice_id, transaction_id, amount,
                payment_date, payment_method, reference, kind
         FROM payments WHERE id = $1 AND ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    row.map(Json)
        .ok_or_else(|| Problem::new(StatusCode::NOT_FOUND, "Not Found", "payment not found"))
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Json(raw): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let body: CreatePaymentBody = serde_json::from_value(raw)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    if !matches!(body.kind.as_str(), "received" | "made") {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "kind must be 'received' or 'made'",
        ));
    }
    if !matches!(
        body.payment_method.as_str(),
        "cash" | "check" | "bank_transfer" | "credit_card" | "other"
    ) {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "invalid payment_method",
        ));
    }
    if body.amount <= Decimal::ZERO {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "amount must be positive",
        ));
    }

    let mut tx = state.pool.begin().await.map_err(db_problem)?;
    let payment_id: Uuid = sqlx::query_scalar(
        "INSERT INTO payments (ledger_id, contact_id, invoice_id, amount, payment_date,
                               payment_method, reference, kind)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(ledger_id)
    .bind(body.contact_id)
    .bind(body.invoice_id)
    .bind(body.amount)
    .bind(body.payment_date)
    .bind(&body.payment_method)
    .bind(body.reference.as_deref())
    .bind(&body.kind)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_problem)?;

    // When linked to an open invoice, apply the payment to it.
    if let Some(invoice_id) = body.invoice_id {
        sqlx::query(
            "UPDATE invoices SET amount_paid = amount_paid + $2, updated_at = now()
             WHERE id = $1 AND ledger_id = $3 AND status IN ('open', 'overdue')",
        )
        .bind(invoice_id)
        .bind(body.amount)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await
        .map_err(db_problem)?;
        sqlx::query(
            "UPDATE invoices SET status = 'paid'
             WHERE id = $1 AND ledger_id = $2 AND amount_paid >= total",
        )
        .bind(invoice_id)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await
        .map_err(db_problem)?;
    }
    tx.commit().await.map_err(db_problem)?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": payment_id })),
    ))
}
