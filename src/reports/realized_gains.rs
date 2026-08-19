//! Realized-gains report (`a6-investment-lots`).
//!
//! Per disposal: when it happened, which lot it consumed,
//! the qty, the proceeds, the cost basis, the gain.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_login::AuthSession;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

/// `GET /ledgers/{id}/reports/realized-gains` — chronological
/// list of disposals with their realized P/L.
pub async fn realized_gains(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let rows: Vec<(
        uuid::Uuid,            // disposal id
        chrono::NaiveDate,     // disposed_at
        uuid::Uuid,            // lot id
        uuid::Uuid,            // account id
        String,                // account name
        rust_decimal::Decimal, // qty
        rust_decimal::Decimal, // unit_proceeds
        rust_decimal::Decimal, // realized_gain
        uuid::Uuid,            // source_txn_id
    )> = sqlx::query_as(
        "SELECT d.id, d.disposed_at, l.id, a.id, a.name,
                d.qty, d.unit_proceeds, d.realized_gain, d.source_txn_id
         FROM investment_disposals d
         JOIN investment_lots l ON l.id = d.lot_id
         JOIN accounts a ON a.id = l.account_id
         JOIN transactions t ON t.id = d.source_txn_id
         WHERE t.ledger_id = $1
            AND t.kind != 'draft'
         ORDER BY d.disposed_at DESC, d.id DESC",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Db)?;

    let body = serde_json::json!({
        "ledger_id": ledger_id,
        "disposals": rows.into_iter().map(|(id, disposed_at, lot_id, account_id, name, qty, unit_proceeds, realized_gain, source_txn_id)| {
            serde_json::json!({
                "id": id,
                "disposed_at": disposed_at,
                "lot_id": lot_id,
                "account_id": account_id,
                "account_name": name,
                "qty": qty,
                "unit_proceeds": unit_proceeds,
                "realized_gain": realized_gain,
                "source_txn_id": source_txn_id,
            })
        }).collect::<Vec<_>>(),
    });
    Ok((StatusCode::OK, axum::Json(body)).into_response())
}
