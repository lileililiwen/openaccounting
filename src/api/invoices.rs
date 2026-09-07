//! Invoice endpoints (`api-v2-coverage`).

use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{
        helpers::{
            db_problem, fingerprint, idempotency_lookup, idempotency_store, next_link, page_from,
            require_access, with_next_link, PageParams,
        },
        problem::Problem,
        ApiTokenId, ApiUser,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/invoices", get(list).post(create))
        .route("/ledgers/{ledger_id}/invoices/{id}", get(get_one))
        .route(
            "/ledgers/{ledger_id}/invoices/{id}/mark-paid",
            post(mark_paid),
        )
        .route("/ledgers/{ledger_id}/invoices/{id}/void", post(void))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InvoiceDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub contact_id: Uuid,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total: Decimal,
    pub amount_paid: Decimal,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvoiceBody {
    pub contact_id: Uuid,
    pub kind: String,
    pub invoice_number: Option<String>,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub lines: Vec<InvoiceLineBody>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InvoiceLineBody {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let page = page_from(&params)?;
    let rows: Vec<InvoiceDto> = sqlx::query_as(
        "SELECT id, ledger_id, contact_id, kind, invoice_number, invoice_date, due_date,
                total, amount_paid, status
         FROM invoices WHERE ledger_id = $1
         ORDER BY invoice_date DESC, created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(ledger_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;
    let link = next_link(
        &format!("/api/v1/ledgers/{ledger_id}/invoices"),
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
) -> Result<Json<serde_json::Value>, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let invoice: Option<InvoiceDto> = sqlx::query_as(
        "SELECT id, ledger_id, contact_id, kind, invoice_number, invoice_date, due_date,
                total, amount_paid, status
         FROM invoices WHERE id = $1 AND ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    let Some(inv) = invoice else {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "invoice not found",
        ));
    };
    let lines: Vec<(String, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "SELECT description, quantity, unit_price, amount FROM invoice_lines
         WHERE invoice_id = $1 ORDER BY sort_order",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;
    Ok(Json(serde_json::json!({
        "invoice": inv,
        "lines": lines.iter().map(|(d, q, up, a)| serde_json::json!({
            "description": d, "quantity": q, "unit_price": up, "amount": a,
        })).collect::<Vec<_>>(),
    })))
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    token: ApiTokenId,
    Path(ledger_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, String), Problem> {
    // Idempotency (durable): replay stored response on key match.
    let idem_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let fp = fingerprint(&body);
    if let Some(k) = &idem_key {
        if let Some((status, resp_body)) = idempotency_lookup(&state.pool, k, token.0, &fp).await? {
            return Ok((
                StatusCode::from_u16(status).unwrap_or(StatusCode::CREATED),
                resp_body,
            ));
        }
    }

    require_access(&state.pool, user.0, ledger_id, true).await?;
    let body: CreateInvoiceBody = serde_json::from_value(body)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    if !matches!(body.kind.as_str(), "receivable" | "payable") {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "kind must be 'receivable' or 'payable'",
        ));
    }
    if body.lines.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "at least one line is required",
        ));
    }
    let total: Decimal = body
        .lines
        .iter()
        .map(|l| (l.quantity * l.unit_price).round_dp(2))
        .sum();

    let mut tx = state.pool.begin().await.map_err(db_problem)?;
    let invoice_id: Uuid = sqlx::query_scalar(
        "INSERT INTO invoices (ledger_id, contact_id, kind, invoice_number, invoice_date, due_date, total)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(ledger_id)
    .bind(body.contact_id)
    .bind(&body.kind)
    .bind(body.invoice_number.as_deref().filter(|s| !s.is_empty()))
    .bind(body.invoice_date)
    .bind(body.due_date)
    .bind(total)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_problem)?;
    for (i, l) in body.lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO invoice_lines (invoice_id, description, quantity, unit_price, amount, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invoice_id)
        .bind(&l.description)
        .bind(l.quantity)
        .bind(l.unit_price)
        .bind((l.quantity * l.unit_price).round_dp(2))
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .map_err(db_problem)?;
    }
    tx.commit().await.map_err(db_problem)?;

    crate::jobs::events::emit(
        &state.pool,
        ledger_id,
        "invoice.created",
        serde_json::json!({ "invoice_id": invoice_id, "kind": body.kind, "total": total }),
    )
    .await;

    let resp = serde_json::json!({ "id": invoice_id, "total": total }).to_string();
    if let Some(k) = idem_key {
        idempotency_store(&state.pool, &k, token.0, &fp, 201, &resp).await?;
    }
    Ok((StatusCode::CREATED, resp))
}

/// Shared state transition used by mark-paid / void.
async fn transition(
    state: &AppState,
    user: ApiUser,
    ledger_id: Uuid,
    id: Uuid,
    new_status: &str,
) -> Result<Json<serde_json::Value>, Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let updated: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE invoices SET status = $3, updated_at = now()
         WHERE id = $1 AND ledger_id = $2 AND status NOT IN ('paid', 'void')
         RETURNING id",
    )
    .bind(id)
    .bind(ledger_id)
    .bind(new_status)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    if updated.is_none() {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "invoice not found or already settled",
        ));
    }
    if new_status == "paid" {
        sqlx::query("UPDATE invoices SET amount_paid = total WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(db_problem)?;
        crate::jobs::events::emit(
            &state.pool,
            ledger_id,
            "invoice.paid",
            serde_json::json!({ "invoice_id": id }),
        )
        .await;
    }
    Ok(Json(serde_json::json!({ "id": id, "status": new_status })))
}

async fn mark_paid(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, Problem> {
    transition(&state, user, ledger_id, id, "paid").await
}

async fn void(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, Problem> {
    transition(&state, user, ledger_id, id, "void").await
}
