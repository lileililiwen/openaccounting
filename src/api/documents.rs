//! Document endpoints (`api-v2-coverage`). Uploads go through the
//! same storage layer and MIME/size validation as the web UI.

use axum::response::IntoResponse;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    routing::{delete as delete_route, get},
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    api::{
        helpers::{db_problem, next_link, page_from, require_access, with_next_link, PageParams},
        problem::Problem,
        ApiUser,
    },
    AppState,
};

/// Same accepted set as the web uploader (`s10-upload-validation`).
const ALLOWED_MIME: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/heic",
    "application/pdf",
    "text/csv",
];
const MAX_BYTES: usize = crate::upload::DEFAULT_MAX_BYTES;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/ledgers/{ledger_id}/documents",
            get(list)
                .post(create)
                .layer(DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
        .route(
            "/ledgers/{ledger_id}/documents/{id}",
            get(download).delete(remove),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DocumentDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub transaction_id: Option<Uuid>,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let page = page_from(&params)?;
    let rows: Vec<DocumentDto> = sqlx::query_as(
        "SELECT d.id, t.ledger_id, d.transaction_id, d.filename, d.mime_type AS mime,
                d.size_bytes AS size, d.uploaded_at AS created_at
         FROM documents d JOIN transactions t ON t.id = d.transaction_id
         WHERE t.ledger_id = $1
         ORDER BY d.uploaded_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(ledger_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;
    let link = next_link(
        &format!("/api/v1/ledgers/{ledger_id}/documents"),
        page,
        rows.len(),
    );
    Ok(with_next_link(
        Json(serde_json::json!({ "data": rows })).into_response(),
        link,
    ))
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;

    let mut transaction_id: Option<Uuid> = None;
    let mut filename: Option<String> = None;
    let mut declared = String::new();
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?
    {
        match field.name().unwrap_or_default() {
            "transaction_id" => {
                let v = field.text().await.map_err(multipart_problem)?;
                transaction_id = Some(Uuid::parse_str(v.trim()).map_err(|_| {
                    Problem::new(StatusCode::BAD_REQUEST, "Bad Request", "bad transaction_id")
                })?);
            }
            "file" => {
                let mut field = field;
                filename = Some(field.file_name().unwrap_or("upload.bin").to_string());
                declared = field
                    .content_type()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "application/octet-stream".into());
                let mut buf = Vec::new();
                let mut total = 0usize;
                loop {
                    let Some(chunk) = field.chunk().await.map_err(multipart_problem)? else {
                        break;
                    };
                    total += chunk.len();
                    if total > MAX_BYTES {
                        return Err(Problem::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            format!("File too large (max {MAX_BYTES} bytes)"),
                        ));
                    }
                    buf.extend_from_slice(&chunk);
                }
                bytes = Some(buf);
            }
            _ => {}
        }
    }
    let Some(filename) = filename else {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "multipart field 'file' is required",
        ));
    };
    let Some(bytes) = bytes else {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "empty upload",
        ));
    };
    let Some(transaction_id) = transaction_id else {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "multipart field 'transaction_id' is required",
        ));
    };

    // The transaction must belong to this ledger.
    let txn_ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM transactions WHERE id = $1 AND ledger_id = $2")
            .bind(transaction_id)
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(db_problem)?;
    if txn_ok.is_none() {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "transaction not found in this ledger",
        ));
    }

    // Same validation path as the web uploader: declared-MIME allowlist
    // + sniff-vs-declared check (`s10-upload-validation`).
    if !ALLOWED_MIME.contains(&declared.as_str()) {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            format!("Unsupported file type: {declared}"),
        ));
    }
    let mime = crate::upload::validate(&declared, Some(&filename), &bytes)
        .map_err(|e| {
            Problem::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                e.message().to_string(),
            )
        })?
        .to_string();

    let storage_key = state
        .storage
        .allocate_path(transaction_id, &filename)
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                e.to_string(),
            )
        })?;
    state
        .storage
        .write(&storage_key, &bytes)
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                e.to_string(),
            )
        })?;
    let stored = match &storage_key {
        crate::storage::StorageKey::Filesystem(p) => p
            .strip_prefix(state.storage.root_for())
            .unwrap_or(p)
            .to_string_lossy()
            .to_string(),
        crate::storage::StorageKey::S3 { key, .. } => key.clone(),
    };

    let (doc_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO documents (transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(transaction_id)
    .bind(ledger_id)
    .bind(&filename)
    .bind(&stored)
    .bind(&mime)
    .bind(bytes.len() as i64)
    .bind(user.0)
    .fetch_one(&state.pool)
    .await
    .map_err(db_problem)?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": doc_id, "filename": filename, "size": bytes.len() })),
    ))
}

async fn download(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT d.filename, d.mime_type, d.stored_filename
         FROM documents d JOIN transactions t ON t.id = d.transaction_id
         WHERE d.id = $1 AND t.ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    let Some((filename, mime, stored)) = row else {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "document not found",
        ));
    };
    let key = state.storage.key_from_stored(&stored).map_err(|e| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            e.to_string(),
        )
    })?;
    let bytes = state.storage.read(&key).await.map_err(|e| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            e.to_string(),
        )
    })?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, mime),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename.replace('"', "")),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn remove(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT d.stored_filename
         FROM documents d JOIN transactions t ON t.id = d.transaction_id
         WHERE d.id = $1 AND t.ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    let Some((stored,)) = row else {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "document not found",
        ));
    };
    let key = state
        .storage
        .key_from_stored(&stored)
        .map_err(db_problem_storage)?;
    state.storage.delete(&key).await.map_err(|e| {
        Problem::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            e.to_string(),
        )
    })?;
    sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(db_problem)?;
    Ok(StatusCode::NO_CONTENT)
}

fn db_problem_storage(e: crate::storage::StorageError) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        e.to_string(),
    )
}

fn multipart_problem(e: axum::extract::multipart::MultipartError) -> Problem {
    Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string())
}
