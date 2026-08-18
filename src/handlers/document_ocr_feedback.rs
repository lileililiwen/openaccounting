//! OCR feedback loop (`o7-ocr-feedback`).
//!
//! After a user applies an OCR result and saves the resulting
//! reimbursement line, this module records the original OCR
//! output alongside the final user-edited values so self-hosters
//! can fine-tune external OCR engines or seed rule-based
//! extractors. The corpus is downloadable from
//! `/admin/ocr-corpus.json` for admins.
//!
//! **Opt-out:** set `OCR_FEEDBACK=false` to disable capture. The
//! apply endpoint still succeeds; only the side-effect row is
//! skipped.
//!
//! **Privacy:** rows hold structured fields only. The original
//! document bytes are never written here.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    AppState,
};

/// One side of an OCR correction — either the engine's output
/// (`ocr_*`) or the final user-edited values (`final_*`).
/// `None` means the engine did not produce that field.
#[derive(Debug, Clone, Serialize)]
pub struct OcrCorrection {
    pub id: Uuid,
    pub document_id: Uuid,
    pub ledger_id: Uuid,
    pub user_id: Uuid,
    pub claim_id: Uuid,
    pub reimbursement_line_id: Uuid,
    pub captured_at: chrono::DateTime<chrono::Utc>,

    pub ocr_amount: Option<Decimal>,
    pub ocr_txn_date: Option<NaiveDate>,
    pub ocr_merchant: Option<String>,
    pub ocr_confidence: Option<f32>,

    pub final_amount: Decimal,
    pub final_txn_date: NaiveDate,
    pub final_merchant: String,
    pub final_account_id: Uuid,
}

/// Returns `true` unless the operator opted out by setting
/// `OCR_FEEDBACK=false`. Checked at apply time so operators can
/// toggle the env var without restarting the binary.
pub fn is_enabled() -> bool {
    !matches!(
        std::env::var("OCR_FEEDBACK").as_deref(),
        Ok("false") | Ok("0") | Ok("no") | Ok("off")
    )
}

/// Inputs required to record one OCR correction.
pub struct RecordInput<'a> {
    pub document_id: Uuid,
    pub ledger_id: Uuid,
    pub user_id: Uuid,
    pub claim_id: Uuid,
    pub reimbursement_line_id: Uuid,
    pub ocr_amount: Option<Decimal>,
    pub ocr_txn_date: Option<NaiveDate>,
    pub ocr_merchant: Option<String>,
    pub ocr_confidence: Option<f32>,
    pub final_amount: Decimal,
    pub final_txn_date: NaiveDate,
    pub final_merchant: &'a str,
    pub final_account_id: Uuid,
}

/// Insert one row into `ocr_corrections`. No-op when
/// `is_enabled()` returns `false`. Failures are logged but do
/// NOT roll back the user's apply — feedback capture is
/// best-effort.
pub async fn record(state: &AppState, input: RecordInput<'_>) {
    if !is_enabled() {
        return;
    }
    let res = sqlx::query(
        r#"INSERT INTO ocr_corrections
               (document_id, ledger_id, user_id, claim_id, reimbursement_line_id,
                ocr_amount, ocr_txn_date, ocr_merchant, ocr_confidence,
                final_amount, final_txn_date, final_merchant, final_account_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
    )
    .bind(input.document_id)
    .bind(input.ledger_id)
    .bind(input.user_id)
    .bind(input.claim_id)
    .bind(input.reimbursement_line_id)
    .bind(input.ocr_amount)
    .bind(input.ocr_txn_date)
    .bind(&input.ocr_merchant)
    .bind(input.ocr_confidence)
    .bind(input.final_amount)
    .bind(input.final_txn_date)
    .bind(input.final_merchant)
    .bind(input.final_account_id)
    .execute(&state.pool)
    .await;
    if let Err(e) = res {
        tracing::warn!(error = %e, "failed to record OCR correction; continuing");
    }
}

/// Admin-only: dump the entire corpus as a JSON array. Newest
/// first. The response is intentionally plain JSON (not
/// pretty-printed) so it can be streamed into a training
/// pipeline. Mounted under `handlers::admin::admin_routes` so
/// the `require_admin` middleware already gates this handler.
pub async fn export_corpus(State(state): State<AppState>) -> AppResult<Response> {
    let rows = sqlx::query(
        r#"SELECT id, document_id, ledger_id, user_id, claim_id,
                  reimbursement_line_id, captured_at,
                  ocr_amount, ocr_txn_date, ocr_merchant, ocr_confidence,
                  final_amount, final_txn_date, final_merchant, final_account_id
           FROM ocr_corrections
           ORDER BY captured_at DESC, id"#,
    )
    .fetch_all(&state.pool)
    .await?;

    let corpus: Vec<OcrCorrection> = rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let document_id: Uuid = row.get("document_id");
            let ledger_id: Uuid = row.get("ledger_id");
            let user_id: Uuid = row.get("user_id");
            let claim_id: Uuid = row.get("claim_id");
            let reimbursement_line_id: Uuid = row.get("reimbursement_line_id");
            let captured_at: chrono::DateTime<chrono::Utc> = row.get("captured_at");
            let ocr_amount: Option<Decimal> = row.try_get("ocr_amount").ok().flatten();
            let ocr_txn_date: Option<NaiveDate> = row.try_get("ocr_txn_date").ok().flatten();
            let ocr_merchant: Option<String> = row.try_get("ocr_merchant").ok().flatten();
            let ocr_confidence: Option<f32> = row.try_get("ocr_confidence").ok().flatten();
            let final_amount: Decimal = row
                .try_get("final_amount")
                .map_err(|e| AppError::Internal(format!("ocr_corrections.final_amount: {e}")))?;
            let final_txn_date: NaiveDate = row
                .try_get("final_txn_date")
                .map_err(|e| AppError::Internal(format!("ocr_corrections.final_txn_date: {e}")))?;
            let final_merchant: String = row
                .try_get("final_merchant")
                .map_err(|e| AppError::Internal(format!("ocr_corrections.final_merchant: {e}")))?;
            let final_account_id: Uuid = row.get("final_account_id");
            Ok::<_, AppError>(OcrCorrection {
                id,
                document_id,
                ledger_id,
                user_id,
                claim_id,
                reimbursement_line_id,
                captured_at,
                ocr_amount,
                ocr_txn_date,
                ocr_merchant,
                ocr_confidence,
                final_amount,
                final_txn_date,
                final_merchant,
                final_account_id,
            })
        })
        .collect::<Result<_, AppError>>()?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"ocr-corpus.json\"",
            ),
        ],
        Json(corpus),
    )
        .into_response())
}

/// Number of rows currently in the corpus. Used by the admin
/// dashboard widget.
pub async fn count(state: &AppState) -> AppResult<i64> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ocr_corrections")
        .fetch_one(&state.pool)
        .await?;
    Ok(n)
}
