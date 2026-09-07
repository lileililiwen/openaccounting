//! Public share links for invoices/estimates (`invoicing-completeness`).
//!
//! Tokens are 256-bit random, stored hashed (SHA-256) like API tokens:
//! a leaked database does not enumerate live links. Revoked links
//! return 410. The public page is login-free, `noindex`, and carries
//! no account data.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::invoice_share::{PublicDocumentPage, SharedDocRow},
    AppState,
};

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Owner action: mint a share link. Renders the list page with the
/// plaintext URL shown once.
pub async fn create_link(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let doc: Option<(String,)> =
        sqlx::query_as("SELECT doc_kind FROM invoices WHERE id = $1 AND ledger_id = $2")
            .bind(invoice_id)
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((doc_kind,)) = doc else {
        return Err(AppError::NotFound);
    };

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    sqlx::query(
        "INSERT INTO invoice_shares (ledger_id, doc_kind, invoice_id, token_hash, created_by)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ledger_id)
    .bind(&doc_kind)
    .bind(invoice_id)
    .bind(hash_token(&token))
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "share",
        doc_kind.as_str(),
        Some(invoice_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!(
        "/ledgers/{ledger_id}/invoices/{invoice_id}?shared={token}"
    ))
    .into_response())
}

pub async fn revoke_link(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query(
        "UPDATE invoice_shares SET revoked_at = now()
         WHERE invoice_id = $1 AND ledger_id = $2 AND revoked_at IS NULL",
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/invoices/{invoice_id}")).into_response())
}

/// Public, unauthenticated document view.
pub async fn public_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let row: Option<(
        Uuid,
        Uuid,
        String,
        String,
        chrono::NaiveDate,
        rust_decimal::Decimal,
        String,
    )> = sqlx::query_as(
        r#"SELECT s.id, i.ledger_id, i.doc_kind, COALESCE(i.invoice_number, ''), i.due_date,
                      i.total, i.status
               FROM invoice_shares s JOIN invoices i ON i.id = s.invoice_id
               WHERE s.token_hash = $1 AND s.revoked_at IS NULL"#,
    )
    .bind(hash_token(&token))
    .fetch_optional(&state.pool)
    .await?;
    // 410 vs 404 distinction: a known-but-revoked hash → 410; unknown → 404.
    if row.is_none() {
        let revoked: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM invoice_shares WHERE token_hash = $1 AND revoked_at IS NOT NULL",
        )
        .bind(hash_token(&token))
        .fetch_optional(&state.pool)
        .await?;
        if revoked.is_some() {
            return Err(AppError::Gone("this share link has been revoked".into()));
        }
        return Err(AppError::NotFound);
    }
    let (_share_id, _ledger_id, doc_kind, number, due_date, total, status) = row.unwrap();
    let is_estimate = doc_kind == "estimate";

    let lines: Vec<SharedDocRow> = sqlx::query_as(
        "SELECT description, quantity, unit_price, amount FROM invoice_lines
         WHERE invoice_id = (SELECT invoice_id FROM invoice_shares WHERE token_hash = $1)
         ORDER BY sort_order",
    )
    .bind(hash_token(&token))
    .fetch_all(&state.pool)
    .await?;

    Ok(crate::templates::render_response(PublicDocumentPage {
        doc_kind,
        number,
        due_date,
        total,
        status,
        lines,
        is_estimate,
        decision: String::new(),
    }))
}

/// Estimate accept/decline through the share link.
#[derive(serde::Deserialize)]
pub struct DecisionForm {
    pub decision: String,
}

pub async fn decide(
    State(state): State<AppState>,
    Path(token): Path<String>,
    axum::Form(form): axum::Form<DecisionForm>,
) -> AppResult<Response> {
    let _ = &state;
    let new_status = match form.decision.as_str() {
        "accept" => "accepted",
        "decline" => "declined",
        _ => return Err(AppError::Validation("invalid decision".into())),
    };
    let updated: Option<(Uuid,)> = sqlx::query_as(
        r#"UPDATE invoices SET status = $2, updated_at = now()
           FROM invoice_shares s
           WHERE s.invoice_id = invoices.id AND s.token_hash = $1
                 AND s.revoked_at IS NULL AND invoices.doc_kind = 'estimate'
                 AND invoices.status IN ('draft', 'sent')
           RETURNING invoices.id"#,
    )
    .bind(hash_token(&token))
    .bind(new_status)
    .fetch_optional(&state.pool)
    .await?;
    if updated.is_none() {
        return Err(AppError::Conflict(
            "this estimate can no longer be acted on".into(),
        ));
    }
    Ok(Redirect::to(&format!("/share/invoice/{token}")).into_response())
}
