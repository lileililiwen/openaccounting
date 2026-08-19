//! WeChat Pay dedicated importer: upload page, preview
//! (parse + dedup against the ledger), and atomic commit.

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
    handlers::ledgers,
    import::{dedup, wechat, ParseOptions, ParsedRow},
    templates::{
        import_wechat::{WechatPreview, WechatUpload},
        render_response,
    },
    AppState,
};

/// Render the WeChat upload page.
pub async fn upload_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(WechatUpload {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "import-wechat".to_string(),
        error: String::new(),
    }))
}

/// Parse an uploaded WeChat bill, mark duplicates against the
/// ledger's recent transactions, and render the preview.
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
    let mut include_pending = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("wechat.csv").to_string();
            bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?
                .to_vec();
        } else if name == "include_pending" {
            include_pending = field.text().await.unwrap_or_default() == "on";
        }
    }

    if bytes.is_empty() {
        return Ok(render_response(WechatUpload {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            current_section: "import-wechat".to_string(),
            error: "No file provided".into(),
        }));
    }

    let options = ParseOptions {
        include_pending,
        ..ParseOptions::default()
    };
    render_wechat_preview(
        &state,
        user,
        ledger_id,
        ledger.name,
        "import-wechat".to_string(),
        &filename,
        &bytes,
        options,
    )
    .await
}

/// Shared preview renderer used by the dedicated upload route
/// and by the generic import endpoint's auto-detect dispatch.
pub(crate) async fn render_wechat_preview(
    state: &AppState,
    user: &crate::auth::User,
    ledger_id: Uuid,
    ledger_name: String,
    current_section: String,
    filename: &str,
    bytes: &[u8],
    options: ParseOptions,
) -> AppResult<Response> {
    let (mut rows, encoding) = wechat::parse(bytes, &options)?;
    let existing = existing_fingerprints(state, ledger_id).await?;
    for row in &mut rows {
        if row.fingerprint != 0 && existing.contains(&row.fingerprint) {
            row.is_duplicate = true;
        }
    }
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import.wechat.parse",
        "import",
        None,
        None,
        Some(json!({ "filename": filename, "rows": rows.len(), "encoding": encoding })),
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

    Ok(render_response(WechatPreview {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name,
        current_section: "import-wechat".to_string(),
        filename: filename.to_string(),
        format: "wechat".to_string(),
        rows,
        rows_json,
        accounts,
        default_account_id: Uuid::nil(),
        include_pending: options.include_pending,
        error: String::new(),
    }))
}

#[derive(serde::Deserialize)]
pub struct WechatCommitForm {
    pub filename: String,
    /// The ledger's default cash account (CR leg).
    pub default_account_id: Uuid,
    /// The user-chosen expense account (DR leg).
    pub expense_account_id: Uuid,
    #[serde(default)]
    pub skip_duplicates: bool,
    /// JSON-encoded `Vec<ParsedRow>` from the preview form.
    pub rows: String,
}

/// Commit the previewed rows atomically.
pub async fn commit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    form: axum::Form<WechatCommitForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rows: Vec<ParsedRow> = if form.rows.trim().is_empty() {
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
        "import.wechat.commit",
        "import",
        None,
        None,
        Some(json!({ "filename": form.filename, "rows_committed": created })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/transactions")).into_response())
}

/// Load the dedup fingerprints of the ledger's transactions
/// from the last 90 days.
pub async fn existing_fingerprints(
    state: &AppState,
    ledger_id: Uuid,
) -> AppResult<std::collections::HashSet<u64>> {
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT t.txn_date::text, p.amount::text, t.payee
           FROM transactions t
           JOIN postings p ON p.transaction_id = t.id
           WHERE t.ledger_id = $1
             AND t.txn_date >= CURRENT_DATE - INTERVAL '90 days'"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let mut set = std::collections::HashSet::new();
    for (date, amount, payee) in rows {
        let Ok(amount) = amount.parse::<rust_decimal::Decimal>() else {
            continue;
        };
        let amount_cents: i64 = (amount * rust_decimal::Decimal::from(100))
            .round()
            .try_into()
            .unwrap_or(0);
        let payee = payee.unwrap_or_default();
        set.insert(dedup::fingerprint(&date, amount_cents, &payee));
    }
    Ok(set)
}
