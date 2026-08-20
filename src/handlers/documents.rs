use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    body::Body,
    extract::{Form, Multipart, Path, Query, State},
    http::{header, StatusCode},
};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    audit,
    auth::{Backend, User},
    domain::Document,
    error::{AppError, AppResult},
    handlers::{document_ocr, ledgers},
    templates::documents::{DocumentBindPage, DocumentList, DocumentWithTxn},
    upload, AppState,
};

#[derive(sqlx::FromRow)]
struct DocWithTxn {
    #[sqlx(flatten)]
    pub doc: Document,
    /// NULL for unbound documents (`a13-document-inbox`).
    pub transaction_date: Option<NaiveDate>,
    /// NULL for unbound documents.
    pub transaction_description: Option<String>,
    /// Derived from `document_ocr_results`: `"done"`, `"failed"`, `"pending"`, or `""`.
    pub ocr_status: String,
}

impl From<DocWithTxn> for DocumentWithTxn {
    fn from(row: DocWithTxn) -> Self {
        DocumentWithTxn {
            id: row.doc.id,
            filename: row.doc.filename,
            mime_type: row.doc.mime_type,
            size_bytes: row.doc.size_bytes,
            uploaded_at: row.doc.uploaded_at,
            transaction_id: row.doc.transaction_id.unwrap_or_default(),
            is_unbound: row.doc.transaction_id.is_none(),
            transaction_date: row.transaction_date.unwrap_or_default(),
            transaction_description: row.transaction_description.unwrap_or_default(),
            ocr_status: row.ocr_status,
        }
    }
}
const MAX_BYTES: usize = upload::DEFAULT_MAX_BYTES; // defense in depth; layer already caps body

/// Sanitize a value for use in HTTP header values.
/// Strips control characters and double-quotes to prevent header injection.
fn sanitize_header_value(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() && *c != '"').collect()
}

/// MIME types accepted by the document upload handler. Anything
/// outside this list is rejected with 400. CSV is allowed on
/// declaration (sniff is unreliable); office / archive formats
/// are auto-corrected by `upload::validate` based on the file
/// extension (see `upload::EXTENSION_WINS`).
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
    // Office / archive: extension-wins in `upload::EXTENSION_WINS`.
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.presentation",
    "application/zip",
];

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let category = q.get("category").cloned().unwrap_or_default();
    let search = q.get("q").cloned().unwrap_or_default();
    let tag = q.get("tag").cloned().unwrap_or_default();
    let from_str = q.get("from").cloned().unwrap_or_default();
    let to_str = q.get("to").cloned().unwrap_or_default();

    let from_date = NaiveDate::parse_from_str(&from_str, "%Y-%m-%d").ok();
    let to_date = NaiveDate::parse_from_str(&to_str, "%Y-%m-%d").ok();

    let rows = sqlx::query_as::<_, DocWithTxn>(
        r#"SELECT d.id, d.transaction_id, d.ledger_id, d.filename, d.stored_filename, d.mime_type, d.size_bytes, d.uploaded_by, d.uploaded_at, d.category,
                  t.txn_date AS transaction_date, t.description AS transaction_description,
                  COALESCE(
                      CASE
                          WHEN o.error_message IS NOT NULL THEN 'failed'
                          WHEN o.id IS NOT NULL             THEN 'done'
                          ELSE ''
                      END, ''
                  ) AS ocr_status
           FROM documents d
           LEFT JOIN transactions t ON t.id = d.transaction_id
           LEFT JOIN document_ocr_results o ON o.document_id = d.id
           WHERE (t.ledger_id = $1 OR d.ledger_id = $1)
                 AND ($2::text IS NULL OR d.filename ILIKE '%' || $2 || '%')
                 AND ($3::text IS NULL OR d.category = $3)
                 AND ($4::text IS NULL OR d.uploaded_at >= $4)
                 AND ($5::text IS NULL OR d.uploaded_at <= $5)
                 AND ($6::text IS NULL OR EXISTS(SELECT 1 FROM document_tags dt WHERE dt.document_id = d.id AND dt.tag = $6))
           ORDER BY d.uploaded_at DESC"#,
    )
    .bind(ledger_id)
    .bind(if search.is_empty() { None } else { Some(search.as_str()) })
    .bind(if category.is_empty() { None } else { Some(category.as_str()) })
    .bind(from_date)
    .bind(to_date)
    .bind(if tag.is_empty() { None } else { Some(tag.as_str()) })
    .fetch_all(&state.pool)
    .await?;
    let documents = rows.into_iter().map(DocumentWithTxn::from).collect();
    Ok(render_response(DocumentList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "documents".to_string(),
        documents,
    }))
}

pub async fn upload(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let skip_ocr = query.get("ocr").map(|v| v == "false").unwrap_or(false);
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
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

    let mut category = String::from("Other");
    let mut tags: Vec<String> = Vec::new();
    let mut count = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "category" {
            let v = field
                .text()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?;
            category = v;
            continue;
        }
        if name == "tags" {
            let v = field
                .text()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?;
            tags = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            continue;
        }
        if name != "file" {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload").to_string();
        let declared = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        if !ALLOWED_MIME.contains(&declared.as_str()) {
            return Err(AppError::Validation(format!(
                "Unsupported file type: {}",
                declared
            )));
        }

        let key = state.storage.allocate_path(txn_id, &filename).await?;
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

        // `s10-upload-validation`: sniff vs declared MIME,
        // accept on declaration for CSV / office-extension
        // overrides, reject otherwise.
        let mime = upload::validate(&declared, Some(&filename), &buf)
            .map_err(|e| AppError::Validation(e.message()))?
            .to_string();
        if !ALLOWED_MIME.contains(&mime.as_str()) {
            return Err(AppError::Validation(format!(
                "Unsupported file type: {}",
                mime
            )));
        }

        state.storage.write(&key, &buf).await?;

        // Derive the relative path used by `documents.stored_filename`.
        // We always store the full backend-relative path so
        // `key_from_stored` can round-trip without needing to
        // know which transaction this object belongs to.
        let stored = match &key {
            crate::storage::StorageKey::Filesystem(p) => p
                .strip_prefix(&state.storage.root_for())
                .unwrap_or(p)
                .to_string_lossy()
                .to_string(),
            crate::storage::StorageKey::S3 { key, .. } => key.clone(),
        };

        let doc_result = sqlx::query(
            r#"INSERT INTO documents (transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id"#,
        )
        .bind(txn_id)
        .bind(ledger_id)
        .bind(&filename)
        .bind(&stored)
        .bind(&mime)
        .bind(total as i64)
        .bind(user.id)
        .bind(&category)
        .fetch_one(&state.pool)
        .await?;

        // Quota warning (observability only — never fails the
        // upload). `s10-upload-validation`.
        upload::maybe_warn_quota(&state.pool, user.id, total as i64).await;

        // Audit log
        let doc_id: Uuid = doc_result.get(0);
        for tag in &tags {
            sqlx::query(
                r#"INSERT INTO document_tags (document_id, tag) VALUES ($1, $2)
                   ON CONFLICT (document_id, tag) DO NOTHING"#,
            )
            .bind(doc_id)
            .bind(tag)
            .execute(&state.pool)
            .await?;
        }

        let _ = audit::log(
            &state.pool,
            Some(ledger_id),
            user.id,
            "create",
            "document",
            Some(doc_id),
            None,
            Some(serde_json::json!({
                "filename": filename,
                "mime_type": mime,
                "size_bytes": total,
                "category": category,
                "tags": tags,
            })),
        )
        .await;

        // Enqueue OCR in background for image and PDF files
        // unless the caller explicitly opts out with ?ocr=false.
        if !skip_ocr && (mime.starts_with("image/") || mime == "application/pdf") {
            document_ocr::enqueue_ocr(state.clone(), doc_id, buf.clone(), mime.clone());
        }

        count += 1;
    }

    if count == 0 {
        return Err(AppError::Validation("No file provided".into()));
    }
    Ok(Redirect::to(&format!("/ledgers/{}/transactions/{}", ledger_id, txn_id)).into_response())
}

/// Upload documents to a ledger without a transaction (the inbox,
/// `a13-document-inbox`). Writer-only. Each uploaded file is stored
/// as an unbound document (`transaction_id` NULL, `ledger_id` set)
/// and can be bound to a transaction later.
pub async fn upload_unbound(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Redirect> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let mut count = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        if field.file_name().is_none() {
            continue;
        }
        let filename = field.file_name().unwrap_or("upload").to_string();
        let declared = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let key = state
            .storage
            .allocate_path(Uuid::new_v4(), &filename)
            .await?;
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

        let mime = upload::validate(&declared, Some(&filename), &buf)
            .map_err(|e| AppError::Validation(e.message()))?
            .to_string();
        if !ALLOWED_MIME.contains(&mime.as_str()) {
            return Err(AppError::Validation(format!(
                "Unsupported file type: {}",
                mime
            )));
        }

        state.storage.write(&key, &buf).await?;
        let stored = match &key {
            crate::storage::StorageKey::Filesystem(p) => p
                .strip_prefix(&state.storage.root_for())
                .unwrap_or(p)
                .to_string_lossy()
                .to_string(),
            crate::storage::StorageKey::S3 { key, .. } => key.clone(),
        };

        let doc_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO documents (transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category)
               VALUES (NULL, $1, $2, $3, $4, $5, $6, 'Other')
               RETURNING id"#,
        )
        .bind(ledger_id)
        .bind(&filename)
        .bind(&stored)
        .bind(&mime)
        .bind(total as i64)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;

        upload::maybe_warn_quota(&state.pool, user.id, total as i64).await;
        let _ = audit::log(
            &state.pool,
            Some(ledger_id),
            user.id,
            "create",
            "document",
            Some(doc_id),
            None,
            Some(serde_json::json!({
                "filename": filename,
                "unbound": true,
            })),
        )
        .await;
        count += 1;
    }

    if count == 0 {
        return Err(AppError::Validation("No file provided".into()));
    }
    Ok(Redirect::to(&format!("/ledgers/{}/documents", ledger_id)))
}

/// Form body for binding an unbound document to a transaction.
#[derive(Deserialize)]
pub struct BindForm {
    pub transaction_id: Uuid,
}

/// Search query for the bind page.
#[derive(Deserialize, Default)]
pub struct BindSearch {
    pub q: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub amount: Option<String>,
}

/// Render `/ledgers/{id}/documents/{doc_id}/bind` — the inbox bind
/// page (`a13-document-inbox`): document info, a transaction search
/// (description / date / amount), and a link to create a transaction.
pub async fn bind_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
    Query(search): Query<BindSearch>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let doc = sqlx::query_as::<_, Document>(
        r#"SELECT id, transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, uploaded_at, category
           FROM documents
           WHERE id = $1 AND ledger_id = $2 AND transaction_id IS NULL"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    // Search transactions in the ledger.
    let amount: Option<Decimal> = search
        .amount
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());
    let from = search
        .from
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let to = search
        .to
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    #[derive(sqlx::FromRow)]
    struct TxnRow {
        id: Uuid,
        date: NaiveDate,
        description: String,
        total: Decimal,
    }
    let results = sqlx::query_as::<_, TxnRow>(
        r#"SELECT * FROM (
               SELECT t.id, t.txn_date AS date, t.description,
                      COALESCE((SELECT SUM(p.amount) FROM postings p
                                WHERE p.transaction_id = t.id AND p.direction = 'DEBIT'), 0) AS total
               FROM transactions t
               WHERE t.ledger_id = $1
           ) s
           WHERE ($2::text IS NULL OR s.description ILIKE '%' || $2 || '%')
             AND ($3::date IS NULL OR s.date = $3)
             AND ($4::date IS NULL OR s.date <= $4)
             AND ($5::numeric IS NULL OR ABS(s.total - $5) < 0.005)
           ORDER BY s.date DESC, s.id
           LIMIT 50"#,
    )
    .bind(ledger_id)
    .bind(search.q.as_deref().filter(|s| !s.is_empty()))
    .bind(from)
    .bind(to)
    .bind(amount)
    .fetch_all(&state.pool)
    .await?;

    let page = DocumentBindPage::new(
        user.clone(),
        ledger_id,
        ledger.name.clone(),
        doc_id,
        doc.filename.clone(),
        crate::templates::documents::BindSearchState {
            q: search.q.unwrap_or_default(),
            from: search.from.unwrap_or_default(),
            to: search.to.unwrap_or_default(),
            amount: search.amount.unwrap_or_default(),
        },
        results
            .into_iter()
            .map(|r| crate::templates::documents::BindTransactionRow {
                id: r.id,
                date: r.date,
                description: r.description,
                total: r.total,
            })
            .collect(),
    );
    Ok(render_response(page))
}

/// Bind an unbound document to a transaction (`a13-document-inbox`).
/// Shared by the bind handler and the create-from-bind flow.
pub(crate) async fn bind_document(
    state: &AppState,
    ledger_id: Uuid,
    doc_id: Uuid,
    transaction_id: Uuid,
    actor: &User,
) -> AppResult<()> {
    // The document must be unbound and belong to this ledger.
    let _: (Uuid,) = sqlx::query_as(
        "SELECT id FROM documents WHERE id = $1 AND ledger_id = $2 AND transaction_id IS NULL",
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    // The transaction must belong to this ledger.
    let _: (Uuid,) = sqlx::query_as("SELECT id FROM transactions WHERE id = $1 AND ledger_id = $2")
        .bind(transaction_id)
        .bind(ledger_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    sqlx::query("UPDATE documents SET transaction_id = $1 WHERE id = $2")
        .bind(transaction_id)
        .bind(doc_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        actor.id,
        "bind",
        "document",
        Some(doc_id),
        Some(serde_json::json!({ "transaction_id": null })),
        Some(serde_json::json!({ "transaction_id": transaction_id })),
    )
    .await;

    Ok(())
}

/// Bind an unbound document to a transaction (`a13-document-inbox`).
/// Writer-only; sets `transaction_id` and audit-logs the bind.
pub async fn bind(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<BindForm>,
) -> AppResult<Redirect> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    bind_document(&state, ledger_id, doc_id, form.transaction_id, user).await?;
    Ok(Redirect::to(&format!("/ledgers/{}/documents", ledger_id)))
}

/// Save a document attached inline during transaction creation
/// (`a12-transaction-entry-ease`). Reuses the upload MIME validation,
/// storage path, quota warning, and audit logging so the inline
/// attach is indistinguishable from a post-create upload.
pub(crate) async fn save_inline_document(
    state: &AppState,
    ledger_id: Uuid,
    txn_id: Uuid,
    user: &User,
    filename: &str,
    declared: &str,
    bytes: &[u8],
) -> AppResult<Uuid> {
    // `s10-upload-validation`: sniff vs declared MIME.
    let mime = upload::validate(declared, Some(filename), bytes)
        .map_err(|e| AppError::Validation(e.message()))?
        .to_string();
    if !ALLOWED_MIME.contains(&mime.as_str()) {
        return Err(AppError::Validation(format!(
            "Unsupported file type: {}",
            mime
        )));
    }

    let key = state.storage.allocate_path(txn_id, filename).await?;
    state.storage.write(&key, bytes).await?;
    let stored = match &key {
        crate::storage::StorageKey::Filesystem(p) => p
            .strip_prefix(&state.storage.root_for())
            .unwrap_or(p)
            .to_string_lossy()
            .to_string(),
        crate::storage::StorageKey::S3 { key, .. } => key.clone(),
    };

    let doc_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO documents (transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, category)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'Other')
           RETURNING id"#,
    )
    .bind(txn_id)
    .bind(ledger_id)
    .bind(filename)
    .bind(&stored)
    .bind(&mime)
    .bind(bytes.len() as i64)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;

    upload::maybe_warn_quota(&state.pool, user.id, bytes.len() as i64).await;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "document",
        Some(doc_id),
        None,
        Some(serde_json::json!({
            "filename": filename,
            "transaction_id": txn_id,
            "inline": true,
        })),
    )
    .await;

    Ok(doc_id)
}

pub async fn download(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    // Download accepts any ledger role (owner / editor / viewer)
    // OR an admin user — for bound documents. Unbound documents
    // (the inbox) are restricted to writers. Cross-ledger or
    // unknown users get 404 to avoid revealing existence.
    let doc = sqlx::query_as::<_, Document>(
        r#"SELECT d.id, d.transaction_id, d.ledger_id, d.filename, d.stored_filename, d.mime_type, d.size_bytes, d.uploaded_by, d.uploaded_at, d.category
           FROM documents d
           LEFT JOIN transactions t ON t.id = d.transaction_id
           WHERE d.id = $1 AND (t.ledger_id = $2 OR d.ledger_id = $2)"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if doc.transaction_id.is_none() {
        // Unbound document: only ledger writers may download.
        let _ = ensure_doc_mutation_access(&state, user, ledger_id).await?;
    } else {
        let _ = ensure_doc_access(&state, user, ledger_id).await?;
    }

    let key = state.storage.key_from_stored(&doc.stored_filename)?;
    let bytes = state.storage.read(&key).await?;
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

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, doc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    // Delete requires editor / owner / admin.
    let _ = ensure_doc_mutation_access(&state, user, ledger_id).await?;

    let doc = sqlx::query_as::<_, Document>(
        r#"SELECT d.id, d.transaction_id, d.ledger_id, d.filename, d.stored_filename, d.mime_type, d.size_bytes, d.uploaded_by, d.uploaded_at, d.category
           FROM documents d
           LEFT JOIN transactions t ON t.id = d.transaction_id
           WHERE d.id = $1 AND (t.ledger_id = $2 OR d.ledger_id = $2)"#,
    )
    .bind(doc_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(doc_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "delete",
        "document",
        Some(doc_id),
        None,
        Some(serde_json::json!({
            "filename": doc.filename,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/documents", ledger_id)).into_response())
}

/// Ensure the requesting user has READ access to the document's
/// ledger: owner, editor, viewer, or admin. Cross-ledger or
/// unknown users get `AppError::NotFound` (404) — never 403, so
/// the existence of documents outside the user's scope is not
/// disclosed.
pub async fn ensure_doc_access(state: &AppState, user: &User, ledger_id: Uuid) -> AppResult<Uuid> {
    if user.role == "Admin" {
        return Ok(user.id);
    }
    let (_, role) = ledgers::ensure_access(state, user.id, ledger_id).await?;
    tracing::debug!(role = %role, "ensure_doc_access OK");
    Ok(user.id)
}

/// Ensure the requesting user has WRITE access to the document's
/// ledger: owner, editor, or admin. Viewers are rejected with
/// `AppError::NotFound` (404, not 403 — same rationale).
pub async fn ensure_doc_mutation_access(
    state: &AppState,
    user: &User,
    ledger_id: Uuid,
) -> AppResult<Uuid> {
    if user.role == "Admin" {
        return Ok(user.id);
    }
    let (_, role) = ledgers::ensure_access(state, user.id, ledger_id).await?;
    if role == "viewer" {
        // Don't reveal that the ledger exists.
        return Err(AppError::NotFound);
    }
    Ok(user.id)
}
