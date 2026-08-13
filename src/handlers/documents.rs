use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
};
use axum_login::AuthSession;
use uuid::Uuid;

use crate::{
    auth::Backend,
    domain::Document,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::documents::{DocumentList, DocumentWithTxn},
    AppState,
};

#[derive(sqlx::FromRow)]
struct DocWithTxn {
    #[sqlx(flatten)]
    pub doc: Document,
    pub transaction_date: chrono::NaiveDate,
    pub transaction_description: String,
}

impl From<DocWithTxn> for DocumentWithTxn {
    fn from(row: DocWithTxn) -> Self {
        DocumentWithTxn {
            id: row.doc.id,
            filename: row.doc.filename,
            mime_type: row.doc.mime_type,
            size_bytes: row.doc.size_bytes,
            uploaded_at: row.doc.uploaded_at,
            transaction_id: row.doc.transaction_id,
            transaction_date: row.transaction_date,
            transaction_description: row.transaction_description,
        }
    }
}
const MAX_BYTES: usize = 25 * 1024 * 1024; // 25 MiB per file

/// Sanitize a value for use in HTTP header values.
/// Strips control characters and double-quotes to prevent header injection.
fn sanitize_header_value(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() && *c != '"').collect()
}
const ALLOWED_MIME: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/heic",
    "image/heif",
    "application/pdf",
    "text/plain",
    "text/csv",
];

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let rows = sqlx::query_as::<_, DocWithTxn>(
        r#"SELECT d.id, d.transaction_id, d.filename, d.stored_filename, d.mime_type, d.size_bytes, d.uploaded_by, d.uploaded_at,
                  t.txn_date AS transaction_date, t.description AS transaction_description
           FROM documents d
           JOIN transactions t ON t.id = d.transaction_id
           WHERE t.ledger_id = $1
           ORDER BY d.uploaded_at DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let documents = rows.into_iter().map(DocumentWithTxn::from).collect();
    Ok(render_response(DocumentList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        documents,
    }))
}

pub async fn upload(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    // Confirm transaction belongs to this ledger.
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT ledger_id FROM transactions WHERE id = $1")
            .bind(txn_id)
            .fetch_optional(&state.pool)
            .await?;
    match owner {
        Some(l) if l == ledger_id => {}
        _ => return Err(AppError::NotFound),
    }

    let mut count = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload").to_string();
        let mime = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        if !ALLOWED_MIME.contains(&mime.as_str()) {
            return Err(AppError::Validation(format!(
                "Unsupported file type: {}",
                mime
            )));
        }

        let path = state.storage.allocate_path(txn_id, &filename)?;
        state.storage.ensure_dir(&path).await?;
        let mut total: usize = 0;
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| AppError::Multipart(e.to_string()))?
        {
            total += chunk.len();
            if total > MAX_BYTES {
                return Err(AppError::Validation(format!(
                    "File too large (max {} bytes)",
                    MAX_BYTES
                )));
            }
            buf.extend_from_slice(&chunk);
        }
        state.storage.write(&path, &buf).await?;

        let stored = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("upload")
            .to_string();

        sqlx::query(
            r#"INSERT INTO documents (transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(txn_id)
        .bind(&filename)
        .bind(&stored)
        .bind(&mime)
        .bind(total as i64)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
        count += 1;
    }

    if count == 0 {
        return Err(AppError::Validation("No file provided".into()));
    }
    Ok(Redirect::to(&format!("/ledgers/{}/transactions/{}", ledger_id, txn_id)).into_response())
}

pub async fn download(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let doc = sqlx::query_as::<_, Document>(
        r#"SELECT d.id, d.transaction_id, d.filename, d.stored_filename, d.mime_type, d.size_bytes, d.uploaded_by, d.uploaded_at
           FROM documents d
           JOIN transactions t ON t.id = d.transaction_id
           WHERE d.id = $1 AND t.ledger_id = $2"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let path = state
        .storage
        .root()
        .join(doc.transaction_id.to_string())
        .join(&doc.stored_filename);
    let bytes = state.storage.read(&path).await?;
    let safe_filename = sanitize_header_value(&doc.filename);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, doc.mime_type.clone())
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", safe_filename),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(resp)
}
