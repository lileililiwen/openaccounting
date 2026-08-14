//! Alipay dedicated importer (mobile + web): upload page,
//! preview (parse + dedup against the ledger), and atomic
//! commit. The mobile and web parsers are auto-detected from
//! the uploaded content.

use axum::{
    extract::{Multipart, Path, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_login::AuthSession;
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::Account,
    error::{AppError, AppResult},
    handlers::{import_wechat::WechatCommitForm, ledgers},
    import::{alipay_mobile, alipay_web, ImportPlatform, ParseOptions},
    templates::{
        import_alipay::{AlipayPreview, AlipayUpload},
        render_response,
    },
    AppState,
};

/// Render the Alipay upload page.
pub async fn upload_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(AlipayUpload {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

/// Parse an uploaded Alipay bill (mobile or web, auto-detected),
/// mark duplicates against the ledger, and render the preview.
pub async fn preview(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut bytes = Vec::new();
    let mut filename = String::new();
    let mut include_other = false;
    let mut include_pending = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("alipay.txt").to_string();
            bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?
                .to_vec();
        } else if name == "include_other" {
            include_other = field.text().await.unwrap_or_default() == "on";
        } else if name == "include_pending" {
            include_pending = field.text().await.unwrap_or_default() == "on";
        }
    }

    if bytes.is_empty() {
        return Ok(render_response(AlipayUpload {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "No file provided".into(),
        }));
    }

    let options = ParseOptions {
        include_other,
        include_pending,
    };
    render_alipay_preview(
        &state,
        user,
        ledger_id,
        ledger.name,
        &filename,
        &bytes,
        options,
    )
    .await
}

/// Shared preview renderer used by the dedicated upload route
/// and by the generic import endpoint's auto-detect dispatch.
pub(crate) async fn render_alipay_preview(
    state: &AppState,
    user: &crate::auth::User,
    ledger_id: Uuid,
    ledger_name: String,
    filename: &str,
    bytes: &[u8],
    options: ParseOptions,
) -> AppResult<Response> {
    let (platform, mut rows, encoding) = if is_web(bytes) {
        let (rows, enc) = alipay_web::parse(bytes, &options)?;
        (ImportPlatform::AlipayWeb, rows, enc)
    } else {
        let (rows, enc) = alipay_mobile::parse(bytes, &options)?;
        (ImportPlatform::AlipayMobile, rows, enc)
    };
    let existing = crate::handlers::import_wechat::existing_fingerprints(state, ledger_id).await?;
    for row in &mut rows {
        if row.fingerprint != 0 && existing.contains(&row.fingerprint) {
            row.is_duplicate = true;
        }
    }
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import.alipay.parse",
        "import",
        None,
        None,
        Some(json!({
            "filename": filename,
            "rows": rows.len(),
            "encoding": encoding,
            "platform": platform.as_str(),
        })),
    )
    .await;

    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let rows_json = serde_json::to_string(&rows)
        .map_err(|e| AppError::Internal(format!("could not serialize rows: {e}")))?;

    Ok(render_response(AlipayPreview {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name,
        filename: filename.to_string(),
        format: platform.as_str().to_string(),
        rows,
        rows_json,
        accounts,
        default_account_id: Uuid::nil(),
        include_other: options.include_other,
        include_pending: options.include_pending,
        error: String::new(),
    }))
}

/// Commit the previewed rows atomically. Reuses the same form
/// shape as the WeChat commit handler.
pub async fn commit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    form: axum::Form<WechatCommitForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rows: Vec<crate::import::ParsedRow> = if form.rows.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&form.rows)
            .map_err(|e| AppError::Validation(format!("could not parse rows: {e}")))?
    };

    let created = crate::handlers::import::insert_rows(
        &state,
        ledger_id,
        user.id,
        &rows,
        form.default_account_id,
        form.expense_account_id,
        form.skip_duplicates,
    )
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import.alipay.commit",
        "import",
        None,
        None,
        Some(json!({ "filename": form.filename, "rows_committed": created })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/transactions")).into_response())
}

/// Distinguish the web `.txt` export (header `交易时间,交易分类`)
/// from the mobile CSV export (header `交易号,商家订单号`).
fn is_web(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    text.contains("交易时间,交易分类")
}
