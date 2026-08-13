use crate::templates::render_response;
use axum::extract::{Multipart, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    domain::Account,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::import::{ImportPreview, ImportUpload},
    AppState,
};

#[derive(Clone, Debug, Default)]
pub struct CsvMapping {
    pub date_column: usize,
    pub description_column: usize,
    pub debit_column: Option<usize>,
    pub credit_column: Option<usize>,
    pub account_column: Option<usize>,
    pub payee_column: Option<usize>,
    pub reference_column: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ParsedRow {
    pub date: String,
    pub description: String,
    pub debit: String,
    pub credit: String,
    pub account: Option<String>,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub is_duplicate: bool,
}

pub async fn upload_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    Ok(render_response(ImportUpload {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

pub async fn upload(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut csv_content = String::new();
    let mut filename = String::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }
        filename = field.file_name().unwrap_or("upload.csv").to_string();
        let chunk = field
            .chunk()
            .await
            .map_err(|e| AppError::Multipart(e.to_string()))?
            .ok_or_else(|| AppError::Validation("Empty file".into()))?;
        csv_content = String::from_utf8(chunk.to_vec())
            .map_err(|_| AppError::Validation("Invalid UTF-8 in CSV file".into()))?;
    }

    if csv_content.is_empty() {
        return Ok(render_response(ImportUpload {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "No file provided".into(),
        }));
    }

    // Parse CSV
    let mut reader = csv::Reader::from_reader(csv_content.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| AppError::Validation(format!("Invalid CSV headers: {}", e)))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();
    for (i, result) in reader.records().enumerate() {
        if i >= 10 {
            break;
        }
        let record = result.map_err(|e| AppError::Validation(format!("CSV row {}: {}", i + 1, e)))?;
        let row = ParsedRow {
            date: record.get(0).unwrap_or("").to_string(),
            description: record.get(1).unwrap_or("").to_string(),
            debit: record.get(2).unwrap_or("").to_string(),
            credit: record.get(3).unwrap_or("").to_string(),
            account: record.get(4).map(|s| s.to_string()),
            payee: record.get(5).map(|s| s.to_string()),
            reference: record.get(6).map(|s| s.to_string()),
            is_duplicate: false,
        };
        rows.push(row);
    }

    // Get accounts for mapping
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(ImportPreview {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        filename,
        headers,
        rows,
        accounts,
        mapping: CsvMapping::default(),
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct ImportConfirmForm {
    pub filename: String,
    pub date_column: usize,
    pub description_column: usize,
    pub debit_column: Option<usize>,
    pub credit_column: Option<usize>,
    pub account_id: Uuid,
    pub skip_duplicates: bool,
}

pub async fn confirm(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    form: axum::Form<ImportConfirmForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // For now, just redirect back with success message
    // Full implementation would parse CSV, validate, and create transactions
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import",
        "transaction",
        None,
        None,
        Some(serde_json::json!({
            "filename": form.filename,
            "account_id": form.account_id,
            "skip_duplicates": form.skip_duplicates
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/transactions", ledger_id)).into_response())
}
