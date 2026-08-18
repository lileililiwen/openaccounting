//! Bulk actions on the transaction list
//! (`u3-bulk-actions`).
//!
//! Five operations are supported via one POST route
//! (\`/ledgers/{id}/transactions/bulk\`):
//!
//! | Action     | Effect                                        |
//! |------------|-----------------------------------------------|
//! | \`tag\`      | add a tag to every selected txn               |
//! | \`untag\`    | remove a tag from every selected txn          |
//! | \`contact\`  | set \`contact_id\` on every selected txn      |
//! | \`delete\`   | insert a reversing transaction (no destructive delete) |
//!
//! All bulk operations reject more than [`MAX_BULK`] selected
//! rows so a runaway form can't lock the DB. The rejecting is
//! a 422 to align with the spec.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

/// Max rows accepted in one bulk action.
pub const MAX_BULK: usize = 500;

#[derive(Deserialize)]
pub struct BulkForm {
    /// Comma-separated transaction ids. We accept the
    /// comma-joined form because serde_urlencoded collapses
    /// repeated keys; see \`u4-csv-import-wizard\` / \`u5\`
    /// for the same workaround.
    pub txn_ids: String,
    pub action: String,
    /// Value the action needs: tag name, contact uuid, or
    /// ignored for delete.
    #[serde(default)]
    pub value: Option<String>,
}

/// POST /ledgers/{id}/transactions/bulk
pub async fn bulk_action(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<BulkForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let ids: Vec<Uuid> = form
        .txn_ids
        .split(',')
        .filter_map(|s| Uuid::parse_str(s.trim()).ok())
        .collect();
    if ids.is_empty() {
        return Err(AppError::Validation(
            "txn_ids must contain at least one uuid".into(),
        ));
    }
    if ids.len() > MAX_BULK {
        return Ok(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("bulk action limited to {MAX_BULK} rows; got {}", ids.len()),
        ));
    }
    let action = form.action.as_str();
    match action {
        "tag" => tag_action(&state, ledger_id, user.id, &ids, form.value.as_deref()).await?,
        "untag" => untag_action(&state, ledger_id, &ids, form.value.as_deref()).await?,
        "contact" => {
            contact_action(&state, ledger_id, user.id, &ids, form.value.as_deref()).await?
        }
        "delete" => delete_action(&state, ledger_id, user.id, &ids).await?,
        other => {
            return Err(AppError::Validation(format!(
                "unknown bulk action: {other}"
            )));
        }
    }

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/transactions")).into_response())
}

// ─── individual actions ────────────────────────────────────────────

async fn tag_action(
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    ids: &[Uuid],
    value: Option<&str>,
) -> AppResult<()> {
    let tag_name = value
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::Validation("tag action requires a value".into()))?;
    let tag_name = tag_name.trim();

    // Upsert the tag row, then bulk-insert into
    // transaction_tags for every selected transaction.
    let mut tx = state.pool.begin().await?;
    let tag_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tags (ledger_id, name) VALUES ($1, $2)
         ON CONFLICT (ledger_id, name) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(tag_name)
    .fetch_one(&mut *tx)
    .await?;

    // Only attach tags to transactions that belong to this
    // ledger (defence in depth — the user already passed the
    // owner check above).
    let mut attached = 0usize;
    for id in ids {
        let n: u64 = sqlx::query(
            "INSERT INTO transaction_tags (transaction_id, tag_id)
             SELECT t.id, $1 FROM transactions t
             WHERE t.id = $2 AND t.ledger_id = $3
             ON CONFLICT DO NOTHING",
        )
        .bind(tag_id)
        .bind(id)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        attached += n as usize;
    }

    let _ = crate::audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "bulk_tag",
        "transactions",
        None,
        None,
        Some(serde_json::json!({
            "tag": tag_name,
            "selected": ids.len(),
            "attached": attached,
        })),
    )
    .await;

    tx.commit().await?;
    Ok(())
}

async fn untag_action(
    state: &AppState,
    ledger_id: Uuid,
    ids: &[Uuid],
    value: Option<&str>,
) -> AppResult<()> {
    let tag_name = value
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::Validation("untag action requires a value".into()))?;
    let tag_name = tag_name.trim();

    let mut tx = state.pool.begin().await?;
    // Resolve the tag id; reject if not present in this ledger
    // so we don't accidentally delete a global tag.
    let tag_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM tags WHERE ledger_id = $1 AND name = $2")
            .bind(ledger_id)
            .bind(tag_name)
            .fetch_optional(&mut *tx)
            .await?;
    let tag_id =
        tag_id.ok_or_else(|| AppError::Validation(format!("tag not found: {tag_name}")))?;

    let mut removed = 0usize;
    for id in ids {
        let n: u64 = sqlx::query(
            "DELETE FROM transaction_tags tt
             USING transactions t
             WHERE tt.transaction_id = t.id
               AND tt.tag_id = $1
               AND t.id = $2
               AND t.ledger_id = $3",
        )
        .bind(tag_id)
        .bind(id)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        removed += n as usize;
    }

    tx.commit().await?;

    let _ = crate::audit::log(
        &state.pool,
        Some(ledger_id),
        Uuid::nil(),
        "bulk_untag",
        "transactions",
        None,
        None,
        Some(serde_json::json!({
            "tag": tag_name,
            "selected": ids.len(),
            "removed": removed,
        })),
    )
    .await;

    Ok(())
}

async fn contact_action(
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    ids: &[Uuid],
    value: Option<&str>,
) -> AppResult<()> {
    let contact_id = match value {
        Some(s) if !s.trim().is_empty() => Uuid::parse_str(s.trim())
            .map_err(|e| AppError::Validation(format!("invalid contact id: {e}")))?,
        _ => {
            return Err(AppError::Validation(
                "contact action requires a value".into(),
            ));
        }
    };

    // Verify the contact belongs to the ledger.
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM contacts WHERE id = $1 AND ledger_id = $2)",
    )
    .bind(contact_id)
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;
    if !ok {
        return Err(AppError::NotFound);
    }

    let mut updated = 0usize;
    for id in ids {
        let n: u64 = sqlx::query(
            "UPDATE transactions
             SET contact_id = $1
             WHERE id = $2 AND ledger_id = $3",
        )
        .bind(contact_id)
        .bind(id)
        .bind(ledger_id)
        .execute(&state.pool)
        .await?
        .rows_affected();
        updated += n as usize;
    }

    let _ = crate::audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "bulk_contact",
        "transactions",
        None,
        None,
        Some(serde_json::json!({
            "contact_id": contact_id.to_string(),
            "selected": ids.len(),
            "updated": updated,
        })),
    )
    .await;

    Ok(())
}

async fn delete_action(
    state: &AppState,
    ledger_id: Uuid,
    user_id: Uuid,
    ids: &[Uuid],
) -> AppResult<()> {
    // Create a reversing transaction for every selected row.
    // The reversal mirrors the original's postings and is
    // flagged with `kind = 'reversing'`. The original is
    // preserved so the audit trail stays intact.
    let mut tx = state.pool.begin().await?;
    let mut reversed = 0usize;
    for orig_id in ids {
        let row: Option<OrigRow> = sqlx::query_as(
            "SELECT t.id, t.ledger_id, t.currency, t.description, t.payee, t.reference, t.txn_date
             FROM transactions t
             WHERE t.id = $1 AND t.ledger_id = $2",
        )
        .bind(orig_id)
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            continue;
        };

        let reversal_id: Uuid = sqlx::query_scalar(
            "INSERT INTO transactions
                 (ledger_id, txn_date, description, payee, reference, currency, created_by, kind)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'reversing')
             RETURNING id",
        )
        .bind(ledger_id)
        .bind(row.txn_date)
        .bind(format!("REVERSAL of {}", row.description))
        .bind(row.payee)
        .bind(row.reference)
        .bind(&row.currency)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
             SELECT $1, account_id, amount,
                    CASE direction WHEN 'DEBIT' THEN 'CREDIT' ELSE 'DEBIT' END,
                    'reversal'
             FROM postings WHERE transaction_id = $2",
        )
        .bind(reversal_id)
        .bind(orig_id)
        .execute(&mut *tx)
        .await?;

        reversed += 1;
    }
    tx.commit().await?;

    let _ = crate::audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "bulk_delete",
        "transactions",
        None,
        None,
        Some(serde_json::json!({
            "selected": ids.len(),
            "reversed": reversed,
            "note": "non-destructive: original rows preserved",
        })),
    )
    .await;

    Ok(())
}

fn error_response(status: StatusCode, msg: &str) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "error": msg,
            "limit": MAX_BULK,
        })),
    )
        .into_response()
}

// Row type extracted so the SELECT in `delete_action` doesn't
// trip clippy's `type_complexity` lint.
#[derive(sqlx::FromRow)]
struct OrigRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    ledger_id: Uuid,
    currency: String,
    description: String,
    payee: Option<String>,
    reference: Option<String>,
    txn_date: chrono::NaiveDate,
}
