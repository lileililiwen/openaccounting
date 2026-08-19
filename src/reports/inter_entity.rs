//! Inter-entity elimination report (`a7-inter-ledger-transfers`).
//!
//! `GET /ledgers/{id}/reports/inter-entity` shows the inter-
//! ledger transfers that touch the given ledger as either the
//! source or the target. When consolidating reports across
//! entities, this list is the set of rows to eliminate.

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
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

/// `GET /ledgers/{id}/reports/inter-entity` — JSON list of the
/// inter-ledger transfers involving this ledger.
pub async fn inter_entity(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let rows: Vec<(
        Uuid,                          // transfer id
        Uuid,                          // from ledger
        Uuid,                          // to ledger
        Uuid,                          // from account
        Uuid,                          // to account
        Decimal,                       // amount
        String,                        // currency
        Uuid,                          // from txn
        Uuid,                          // to txn
        Decimal,                       // fee amount
        chrono::DateTime<chrono::Utc>, // created_at
    )> = sqlx::query_as(
        "SELECT id, from_ledger_id, to_ledger_id, from_account_id, to_account_id,
                amount, currency, from_txn_id, to_txn_id, fee_amount, created_at
         FROM inter_ledger_transfers
         WHERE from_ledger_id = $1 OR to_ledger_id = $1
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Db)?;

    let body = serde_json::json!({
        "ledger_id": ledger_id,
        "transfers": rows.into_iter().map(|(id, from_l, to_l, from_a, to_a, amount, currency, from_t, to_t, fee, created_at)| {
            serde_json::json!({
                "id": id,
                "from_ledger_id": from_l,
                "to_ledger_id": to_l,
                "from_account_id": from_a,
                "to_account_id": to_a,
                "amount": amount,
                "currency": currency,
                "from_txn_id": from_t,
                "to_txn_id": to_t,
                "fee_amount": fee,
                "created_at": created_at,
            })
        }).collect::<Vec<_>>(),
    });
    Ok((StatusCode::OK, axum::Json(body)).into_response())
}
