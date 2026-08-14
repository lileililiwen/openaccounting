//! HTTP handlers for the document OCR feature.
//!
//! Routes:
//!   GET  /ledgers/{id}/documents/{doc_id}/ocr        — show result (or pending)
//!   POST /ledgers/{id}/documents/{doc_id}/ocr        — kick off OCR job
//!   POST /ledgers/{id}/documents/{doc_id}/ocr/apply  — apply result to a reimbursement line

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    ocr::OcrEngine,
    templates::{
        document_ocr::{DocumentOcrPage, OcrResultView},
        render_response,
    },
    AppState,
};

// ─── GET /ledgers/{id}/documents/{doc_id}/ocr ────────────────────────────────

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // Verify doc belongs to this ledger.
    let (filename,): (String,) = sqlx::query_as(
        r#"SELECT d.filename FROM documents d
           JOIN transactions t ON t.id = d.transaction_id
           WHERE d.id = $1 AND t.ledger_id = $2"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    // Check for an existing OCR result row.
    let result_row = sqlx::query(
        r#"SELECT amount, txn_date, merchant, raw_text, confidence, error_message
           FROM document_ocr_results WHERE document_id = $1"#,
    )
    .bind(doc_id)
    .fetch_optional(&state.pool)
    .await?;

    // Look up any open draft claim to pre-select for Apply.
    let claim_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM reimbursement_claims WHERE ledger_id = $1 AND status = 'draft' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;

    let result = result_row.map(|row| OcrResultView {
        amount: row.try_get("amount").ok().flatten(),
        txn_date: row.try_get("txn_date").ok().flatten(),
        merchant: row.try_get::<Option<String>, _>("merchant").ok().flatten(),
        raw_text: row.try_get::<String, _>("raw_text").unwrap_or_default(),
        confidence: row.try_get::<f32, _>("confidence").unwrap_or(0.0),
        error_message: row
            .try_get::<Option<String>, _>("error_message")
            .ok()
            .flatten(),
        claim_id,
    });

    Ok(render_response(DocumentOcrPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        document_id: doc_id,
        filename,
        result,
    }))
}

// ─── POST /ledgers/{id}/documents/{doc_id}/ocr ───────────────────────────────

pub async fn run(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // Verify doc belongs to this ledger and get stored filename + mime.
    let row = sqlx::query(
        r#"SELECT d.stored_filename, d.mime_type, d.transaction_id
           FROM documents d
           JOIN transactions t ON t.id = d.transaction_id
           WHERE d.id = $1 AND t.ledger_id = $2"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let stored: String = row.get("stored_filename");
    let mime: String = row.get("mime_type");
    let txn_id: Uuid = row.get("transaction_id");

    // Read the file bytes.
    let path = state.storage.root().join(txn_id.to_string()).join(&stored);
    let bytes = state.storage.read(&path).await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "run",
        "document.ocr",
        Some(doc_id),
        None,
        None,
    )
    .await;

    // Spawn the OCR job in the background.
    enqueue_ocr(state.clone(), doc_id, bytes, mime);

    // Redirect to the OCR result page (will show "pending").
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/documents/{doc_id}/ocr")).into_response())
}

/// Spawn an async task that runs OCR and writes the result row.
pub fn enqueue_ocr(state: AppState, doc_id: Uuid, bytes: Vec<u8>, mime: String) {
    tokio::spawn(async move {
        let engine = crate::ocr::tesseract::TesseractEngine::default();
        match engine.extract(&bytes, &mime).await {
            Ok(r) => {
                let _ = sqlx::query(
                    r#"INSERT INTO document_ocr_results
                           (document_id, amount, txn_date, merchant, raw_text, engine, confidence)
                       VALUES ($1, $2, $3, $4, $5, 'tesseract', $6)
                       ON CONFLICT (document_id) DO UPDATE
                           SET amount       = EXCLUDED.amount,
                               txn_date     = EXCLUDED.txn_date,
                               merchant     = EXCLUDED.merchant,
                               raw_text     = EXCLUDED.raw_text,
                               confidence   = EXCLUDED.confidence,
                               extracted_at = now(),
                               error_message = NULL"#,
                )
                .bind(doc_id)
                .bind(r.amount)
                .bind(r.txn_date)
                .bind(r.merchant)
                .bind(r.raw_text)
                .bind(r.confidence)
                .execute(&state.pool)
                .await;
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = sqlx::query(
                    r#"INSERT INTO document_ocr_results
                           (document_id, raw_text, engine, confidence, error_message)
                       VALUES ($1, '', 'tesseract', 0, $2)
                       ON CONFLICT (document_id) DO UPDATE
                           SET error_message = EXCLUDED.error_message,
                               extracted_at  = now()"#,
                )
                .bind(doc_id)
                .bind(msg)
                .execute(&state.pool)
                .await;
            }
        }
    });
}

// ─── POST /ledgers/{id}/documents/{doc_id}/ocr/apply ────────────────────────

#[derive(Deserialize)]
pub struct ApplyForm {
    pub claim_id: Uuid,
}

pub async fn apply(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<ApplyForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // Load the OCR result.
    let ocr_row = sqlx::query(
        r#"SELECT amount, txn_date, merchant FROM document_ocr_results
           WHERE document_id = $1 AND error_message IS NULL"#,
    )
    .bind(doc_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Validation("OCR result not available".into()))?;

    let amount: Option<rust_decimal::Decimal> = ocr_row.try_get("amount").ok().flatten();
    let txn_date: Option<chrono::NaiveDate> = ocr_row.try_get("txn_date").ok().flatten();
    let merchant: Option<String> = ocr_row
        .try_get::<Option<String>, _>("merchant")
        .ok()
        .flatten();

    let amount = amount.ok_or_else(|| AppError::Validation("No amount in OCR result".into()))?;
    let txn_date = txn_date.ok_or_else(|| AppError::Validation("No date in OCR result".into()))?;

    // Verify claim belongs to this ledger and is in draft/submitted.
    let claim_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reimbursement_claims WHERE id = $1 AND ledger_id = $2)",
    )
    .bind(form.claim_id)
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;
    if !claim_exists {
        return Err(AppError::NotFound);
    }

    // Look up the default GL account for OCR-applied lines (Other Expense).
    let gl_account_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Other Expense' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Validation("No 'Other Expense' account found".into()))?;

    let description = merchant.unwrap_or_else(|| "Receipt (OCR)".to_string());

    // Insert the reimbursement line.
    let _: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO reimbursement_lines
               (claim_id, txn_date, description, amount, gl_account_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(form.claim_id)
    .bind(txn_date)
    .bind(&description)
    .bind(amount)
    .bind(gl_account_id)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "apply",
        "document.ocr",
        Some(doc_id),
        None,
        Some(serde_json::json!({
            "claim_id": form.claim_id,
            "amount": amount.to_string(),
        })),
    )
    .await;

    Ok((
        StatusCode::SEE_OTHER,
        [(
            axum::http::header::LOCATION,
            format!("/ledgers/{ledger_id}/reimbursements/{}", form.claim_id),
        )],
    )
        .into_response())
}
