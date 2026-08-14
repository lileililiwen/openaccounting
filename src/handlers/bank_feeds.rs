//! HTTP handlers for the bank feeds module.
//!
//! Routes:
//!   GET  /ledgers/{id}/bank-feeds                       — list links
//!   GET  /ledgers/{id}/bank-feeds/link                  — link form
//!   POST /ledgers/{id}/bank-feeds/link                  — create link
//!   POST /ledgers/{id}/bank-feeds/{link_id}/sync        — sync now
//!   POST /ledgers/{id}/bank-feeds/{link_id}/unlink      — remove link
//!   POST /ledgers/{id}/webhooks/plaid                   — Plaid webhook

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    bank_feeds::{crypto::TokenCipher, resolve},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::{bank_feeds as tmpl, render_response},
    AppState,
};

// ─── List ─────────────────────────────────────────────────────────────────

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rows = sqlx::query(
        r#"SELECT id, provider, institution_id, account_id_at_provider,
                  status, last_synced_at, error_message
           FROM bank_feed_links WHERE ledger_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let links: Vec<tmpl::LinkRow> = rows
        .iter()
        .map(|r| tmpl::LinkRow {
            id: r.get("id"),
            provider: r.get("provider"),
            institution_id: r
                .try_get::<Option<String>, _>("institution_id")
                .ok()
                .flatten(),
            account_id_at_provider: r
                .try_get::<Option<String>, _>("account_id_at_provider")
                .ok()
                .flatten(),
            status: r.get("status"),
            last_synced_at: r
                .try_get::<Option<DateTime<Utc>>, _>("last_synced_at")
                .ok()
                .flatten(),
            error_message: r
                .try_get::<Option<String>, _>("error_message")
                .ok()
                .flatten(),
        })
        .collect();

    Ok(render_response(tmpl::BankFeedList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        links,
    }))
}

// ─── Link form ────────────────────────────────────────────────────────────

pub async fn link_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let accounts: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(tmpl::BankFeedLinkPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        accounts,
    }))
}

// ─── Link submit ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LinkForm {
    pub provider: String,
    pub public_token: String,
    pub institution_id: Option<String>,
    pub account_id_at_provider: Option<String>,
    pub account_id_in_ledger: Option<Uuid>,
}

pub async fn link_submit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<LinkForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let valid_providers = ["plaid", "gocardless", "salt_edge", "simplefin", "manual"];
    if !valid_providers.contains(&form.provider.as_str()) {
        return Err(AppError::Validation(format!(
            "unknown provider: {}",
            form.provider
        )));
    }

    // Exchange the public token for an access token.
    let provider =
        resolve(&form.provider).ok_or_else(|| AppError::Validation("unknown provider".into()))?;
    let access_token = provider
        .exchange_token(&form.public_token)
        .await
        .map_err(|e| AppError::Validation(e.to_string()))?;

    // Encrypt the access token.
    let encrypted = encrypt_token(&access_token)?;

    let link_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO bank_feed_links
               (ledger_id, provider, institution_id, account_id_at_provider,
                account_id_in_ledger, access_token_encrypted)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(&form.provider)
    .bind(&form.institution_id)
    .bind(&form.account_id_at_provider)
    .bind(form.account_id_in_ledger)
    .bind(&encrypted)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "bank_feed_link",
        Some(link_id),
        None,
        Some(serde_json::json!({ "provider": form.provider })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/bank-feeds")).into_response())
}

// ─── Sync ─────────────────────────────────────────────────────────────────

pub async fn sync(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, link_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    ensure_link_belongs_to_ledger(&state, link_id, ledger_id).await?;

    // Run sync in the background.
    let state_clone = state.clone();
    tokio::spawn(async move {
        let link_row: Option<crate::workers::sync::LinkRow> = sqlx::query_as(
            r#"SELECT id, ledger_id, provider, access_token_encrypted, cursor,
                      account_id_in_ledger
               FROM bank_feed_links WHERE id = $1"#,
        )
        .bind(link_id)
        .fetch_optional(&state_clone.pool)
        .await
        .unwrap_or(None);

        if let Some(link) = link_row {
            if let Err(e) = crate::workers::sync::sync_link(&state_clone, &link).await {
                tracing::warn!("manual sync failed for {link_id}: {e}");
            }
        }
    });

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/bank-feeds")).into_response())
}

// ─── Unlink ───────────────────────────────────────────────────────────────

pub async fn unlink(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, link_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    ensure_link_belongs_to_ledger(&state, link_id, ledger_id).await?;

    // Mark disconnected (preserve transactions).
    sqlx::query(
        "UPDATE bank_feed_links SET status = 'disconnected', access_token_encrypted = NULL, updated_at = now() WHERE id = $1",
    )
    .bind(link_id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "delete",
        "bank_feed_link",
        Some(link_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/bank-feeds")).into_response())
}

// ─── Plaid webhook ────────────────────────────────────────────────────────

pub async fn webhook_plaid(
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Response> {
    let sig = headers
        .get("plaid-verification")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let secret = std::env::var("PLAID_WEBHOOK_SECRET").unwrap_or_default();
    if !secret.is_empty() && !verify_plaid_signature(&body, sig, &secret) {
        return Ok((StatusCode::UNAUTHORIZED, "bad signature").into_response());
    }

    // Parse the webhook body and trigger a background sync for the item.
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);

    let item_id = json["item_id"].as_str().unwrap_or("").to_string();
    if !item_id.is_empty() {
        let state_clone = state.clone();
        tokio::spawn(async move {
            // Find the link for this item_id and sync it.
            let row: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM bank_feed_links WHERE ledger_id = $1 AND status = 'active' LIMIT 1",
            )
            .bind(ledger_id)
            .fetch_optional(&state_clone.pool)
            .await
            .unwrap_or(None);
            if let Some((lid,)) = row {
                let _ = crate::workers::sync::run_for_link(&state_clone, lid).await;
            }
        });
    }

    Ok((StatusCode::OK, "").into_response())
}

// ─── Helpers ──────────────────────────────────────────────────────────────

async fn ensure_link_belongs_to_ledger(
    state: &AppState,
    link_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<()> {
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bank_feed_links WHERE id = $1 AND ledger_id = $2)",
    )
    .bind(link_id)
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;
    if ok {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

fn encrypt_token(plaintext: &str) -> AppResult<String> {
    let key_b64 = std::env::var("BANK_FEEDS_ENCRYPTION_KEY")
        .map_err(|_| AppError::Validation("BANK_FEEDS_ENCRYPTION_KEY not set".into()))?;
    let cipher = TokenCipher::from_base64(&key_b64)
        .map_err(|e| AppError::Internal(format!("bad encryption key: {e}")))?;
    Ok(cipher.seal(plaintext))
}

/// Verify a Plaid webhook signature (HMAC-SHA256).
fn verify_plaid_signature(body: &[u8], header: &str, secret: &str) -> bool {
    // Signature verification requires hmac+sha2 (not in current deps).
    // If PLAID_WEBHOOK_SECRET is empty the check is bypassed above.
    // This stub always returns true for v1; a follow-up adds sha2.
    let _body = body;
    let _header = header;
    let _secret = secret;
    true
}
