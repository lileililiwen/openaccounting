//! Draft transaction lifecycle (`a8-draft-transactions`).
//!
//! - `GET /ledgers/{id}/drafts` lists all drafts.
//! - `POST /ledgers/{id}/transactions/{txn}/post` promotes a draft.
//! - `POST /ledgers/{id}/transactions/{txn}/discard` deletes a draft
//!   (without producing a reversal entry).
//!
//! Promotion runs `PostingService::post_draft`, which re-applies the
//! period-close check and writes the audit log. Discard runs
//! `PostingService::delete_draft`, which refuses to delete anything
//! that is not `kind='draft'`.

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{routing::get, Form, Router};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    domain::posting_service::{PostingService, PostingServiceError},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::transactions_draft::DraftsPage,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/drafts", get(list_drafts))
        .route(
            "/ledgers/{ledger_id}/transactions/{txn_id}/post",
            axum::routing::post(post_draft),
        )
        .route(
            "/ledgers/{ledger_id}/transactions/{txn_id}/discard",
            axum::routing::post(discard_draft),
        )
}

/// All draft rows for a ledger, newest first. Excludes posted /
/// standard / adjusting / closing / reversing transactions.
pub async fn list_drafts(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let rows = sqlx::query_as::<_, (Uuid, NaiveDate, String, String, Option<String>, Decimal)>(
        r#"SELECT t.id, t.txn_date, t.description, t.currency, t.payee,
                  COALESCE((SELECT SUM(p.amount) FROM postings p
                            WHERE p.transaction_id = t.id AND p.direction='DEBIT'), 0) AS total
           FROM transactions t
           WHERE t.ledger_id = $1 AND t.kind = 'draft'
           ORDER BY t.txn_date DESC, t.created_at DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let drafts: Vec<crate::templates::transactions_draft::DraftRow> = rows
        .into_iter()
        .map(|(id, date, description, currency, payee, total)| {
            crate::templates::transactions_draft::DraftRow {
                id,
                date,
                description,
                currency,
                payee: payee.unwrap_or_default(),
                total,
            }
        })
        .collect();

    Ok(render_response(DraftsPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        drafts,
        flash: String::new(),
    }))
}

/// Promote a draft to posted. The service re-applies the
/// period-close check; an error here surfaces as a 422.
pub async fn post_draft(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    Form(_form): Form<PostDraftForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    match PostingService::post_draft(&state.pool, ledger_id, txn_id, user.id).await {
        Ok(_) => Ok(
            Redirect::to(&format!("/ledgers/{ledger_id}/transactions/{txn_id}")).into_response(),
        ),
        Err(PostingServiceError::Unbalanced { .. }) => Err(AppError::Unprocessable(
            "this is not a draft transaction".into(),
        )),
        Err(PostingServiceError::PeriodClosed { year, .. }) => Err(AppError::Validation(format!(
            "Period {year} is closed; drafts cannot be promoted into closed periods"
        ))),
        Err(PostingServiceError::LedgerNotFound) => Err(AppError::NotFound),
        Err(PostingServiceError::UnknownAccount(_)) => Err(AppError::NotFound),
        Err(PostingServiceError::WrongLedger(_)) => Err(AppError::NotFound),
        Err(PostingServiceError::DuplicateNumber { .. }) => Err(AppError::Conflict(
            "promotion would create a duplicate number".into(),
        )),
        Err(PostingServiceError::MissingFxRate(msg)) => Err(AppError::Validation(msg)),
        Err(PostingServiceError::Db(e)) => Err(AppError::Db(e)),
    }
}

/// Discard a draft. Refuses anything that is not a draft.
#[derive(Deserialize, Default)]
pub struct PostDraftForm {
    #[serde(default)]
    pub _unused: Option<String>,
}

pub async fn discard_draft(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    Form(_form): Form<PostDraftForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    match PostingService::delete_draft(&state.pool, ledger_id, txn_id, user.id).await {
        Ok(_) => Ok(Redirect::to(&format!("/ledgers/{ledger_id}/drafts")).into_response()),
        Err(PostingServiceError::Unbalanced { .. }) => Err(AppError::Unprocessable(
            "only draft transactions can be discarded; reverse the original instead".into(),
        )),
        Err(PostingServiceError::LedgerNotFound) => Err(AppError::NotFound),
        Err(PostingServiceError::UnknownAccount(_)) => Err(AppError::NotFound),
        Err(PostingServiceError::WrongLedger(_)) => Err(AppError::NotFound),
        Err(PostingServiceError::PeriodClosed { .. }) => {
            Err(AppError::Validation("period closed".into()))
        }
        Err(PostingServiceError::MissingFxRate(msg)) => Err(AppError::Validation(msg)),
        Err(PostingServiceError::DuplicateNumber { .. }) => {
            Err(AppError::Conflict("discard would conflict".into()))
        }
        Err(PostingServiceError::Db(e)) => Err(AppError::Db(e)),
    }
}
