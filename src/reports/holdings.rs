//! Holdings report (`a6-investment-lots`).
//!
//! Per investment account: remaining qty, total cost basis of
//! open lots. (Market value is out of scope for v1 — no auto-quote.)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_login::AuthSession;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    auth::Backend,
    domain::investment_lot,
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

/// `GET /ledgers/{id}/reports/holdings` — returns a
/// per-investment-account breakdown of remaining qty + cost.
pub async fn holdings(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    // Per-account aggregation. The account's `type` column
    // identifies investment accounts (e.g. ASSET with subtype
    // like INVESTMENT).
    let rows: Vec<(Uuid, String, String, Option<Decimal>, Option<Decimal>, String)> = sqlx::query_as(
        "SELECT a.id, a.name, a.type,
                (SELECT COALESCE(SUM(l.qty - COALESCE(d_sum.qty_sum, 0)), 0)::DECIMAL
                 FROM investment_lots l
                 LEFT JOIN (
                     SELECT lot_id, SUM(qty) AS qty_sum
                     FROM investment_disposals GROUP BY lot_id
                 ) d_sum ON d_sum.lot_id = l.id
                 WHERE l.account_id = a.id) AS qty,
                (SELECT COALESCE(SUM((l.qty - COALESCE(d_sum.qty_sum, 0)) * l.unit_cost), 0)::DECIMAL
                 FROM investment_lots l
                 LEFT JOIN (
                     SELECT lot_id, SUM(qty) AS qty_sum
                     FROM investment_disposals GROUP BY lot_id
                 ) d_sum ON d_sum.lot_id = l.id
                 WHERE l.account_id = a.id
                   AND (l.qty - COALESCE(d_sum.qty_sum, 0)) > 0) AS cost,
                a.currency
         FROM accounts a
         WHERE a.ledger_id = $1
           AND EXISTS (
               SELECT 1 FROM investment_lots l WHERE l.account_id = a.id
           )
         ORDER BY a.name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::Db(e))?;

    let body = serde_json::json!({
        "ledger_id": ledger_id,
        "holdings": rows.into_iter().map(|(id, name, ty, qty, cost, currency)| {
            serde_json::json!({
                "account_id": id,
                "name": name,
                "type": ty,
                "qty": qty.unwrap_or(Decimal::ZERO),
                "cost_basis": cost.unwrap_or(Decimal::ZERO),
                "currency": currency,
            })
        }).collect::<Vec<_>>(),
    });

    Ok((StatusCode::OK, axum::Json(body)).into_response())
}

/// Touch the import to silence unused warnings if this file is
/// the only consumer of `investment_lot` from outside the domain
/// module.
#[allow(dead_code)]
fn _touch(_: &investment_lot::InvestmentLot) {}
