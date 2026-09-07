//! Shared API helpers (`api-v2-coverage`): role-based ledger access,
//! signed cursor pagination, and the durable idempotency store.

use axum::http::{header, HeaderValue, StatusCode};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::problem::Problem;

// ── Access control ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerRole {
    Owner,
    Editor,
    Viewer,
}

/// Resolve the caller's role on a ledger: explicit owner, or the
/// role recorded in `ledger_members`. `None` = no relationship.
pub async fn ledger_role_for(
    pool: &PgPool,
    user_id: Uuid,
    ledger_id: Uuid,
) -> Result<Option<LedgerRole>, sqlx::Error> {
    let owner: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM ledgers WHERE id = $1 AND owner_id = $2")
            .bind(ledger_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    if owner.is_some() {
        return Ok(Some(LedgerRole::Owner));
    }
    let member: Option<(String,)> =
        sqlx::query_as("SELECT role FROM ledger_members WHERE ledger_id = $1 AND user_id = $2")
            .bind(ledger_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(member.map(|(role,)| match role.as_str() {
        "editor" => LedgerRole::Editor,
        _ => LedgerRole::Viewer,
    }))
}

/// 404 when the caller has no access at all (do not leak existence),
/// 403 when the role is insufficient.
pub fn check_access(role: Option<LedgerRole>, need_write: bool) -> Result<LedgerRole, Problem> {
    let Some(role) = role else {
        return Err(Problem::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "ledger not found",
        ));
    };
    if need_write && matches!(role, LedgerRole::Viewer) {
        return Err(Problem::new(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "viewer tokens cannot modify this ledger",
        )
        .with_type("/errors/forbidden"));
    }
    Ok(role)
}

/// Convenience: resolve + check in one call.
pub async fn require_access(
    pool: &PgPool,
    user_id: Uuid,
    ledger_id: Uuid,
    need_write: bool,
) -> Result<LedgerRole, Problem> {
    let role = ledger_role_for(pool, user_id, ledger_id)
        .await
        .map_err(|e| {
            Problem::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                e.to_string(),
            )
        })?;
    check_access(role, need_write)
}

// ── Signed cursor pagination ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PageParams {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub limit: i64,
    pub offset: i64,
}

impl Page {
    pub const DEFAULT_LIMIT: i64 = 50;
    pub const MAX_LIMIT: i64 = 200;
}

fn signing_key() -> Vec<u8> {
    let secret = std::env::var("APP_SECRET").unwrap_or_else(|_| "dev-secret".into());
    let mut mac =
        <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(b"oa-api-cursor-v1");
    mac.finalize().into_bytes().to_vec()
}

fn sign_offset(offset: i64) -> String {
    let mut mac = <Hmac<Sha256>>::new_from_slice(&signing_key()).expect("key ok");
    mac.update(format!("cursor:{offset}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Parse `?limit=&cursor=`. Forged or malformed cursors fail closed
/// with 400.
pub fn page_from(params: &PageParams) -> Result<Page, Problem> {
    let limit = params.limit.unwrap_or(Page::DEFAULT_LIMIT);
    if !(1..=Page::MAX_LIMIT).contains(&limit) {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            format!("limit must be between 1 and {}", Page::MAX_LIMIT),
        ));
    }
    let offset = match &params.cursor {
        None => 0,
        Some(raw) => {
            let Some((offset_part, sig)) = raw.split_once('.') else {
                return Err(Problem::new(
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "malformed cursor",
                ));
            };
            let offset: i64 = offset_part.parse().map_err(|_| {
                Problem::new(StatusCode::BAD_REQUEST, "Bad Request", "malformed cursor")
            })?;
            if offset < 0 || sign_offset(offset) != sig {
                return Err(Problem::new(
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "invalid cursor signature",
                ));
            }
            offset
        }
    };
    Ok(Page { limit, offset })
}

/// Build the `Link: <…>; rel="next"` header value when the page is full.
pub fn next_link(path_and_query_prefix: &str, page: Page, returned: usize) -> Option<HeaderValue> {
    if (returned as i64) < page.limit {
        return None;
    }
    let next_offset = page.offset + returned as i64;
    let cursor = format!("{next_offset}.{}", sign_offset(next_offset));
    let sep = if path_and_query_prefix.contains('?') {
        '&'
    } else {
        '?'
    };
    HeaderValue::from_str(&format!(
        "<{path_and_query_prefix}{sep}limit={}&cursor={cursor}>; rel=\"next\"",
        page.limit
    ))
    .ok()
}

/// Attach the Link header to a response.
pub fn with_next_link(
    mut resp: axum::response::Response,
    link: Option<HeaderValue>,
) -> axum::response::Response {
    if let Some(v) = link {
        resp.headers_mut().insert(header::LINK, v);
    }
    resp
}

// ── Durable idempotency ─────────────────────────────────────────────────

pub const IDEMPOTENCY_WINDOW_HOURS: i32 = 24;

/// Look up a stored response for `(key, token)` within the replay
/// window. A fingerprint mismatch is a hard 422 per spec.
pub async fn idempotency_lookup(
    pool: &PgPool,
    key: &str,
    token_id: Uuid,
    fingerprint: &str,
) -> Result<Option<(u16, String)>, Problem> {
    let row: Option<(i32, String, String)> = sqlx::query_as(
        "SELECT response_status, response_body, request_fingerprint
         FROM api_idempotency
         WHERE key = $1 AND token_id = $2
               AND created_at > now() - make_interval(hours => $3)",
    )
    .bind(key)
    .bind(token_id)
    .bind(IDEMPOTENCY_WINDOW_HOURS)
    .fetch_optional(pool)
    .await
    .map_err(db_problem)?;
    match row {
        None => Ok(None),
        Some((status, body, stored_fp)) => {
            if stored_fp != fingerprint {
                return Err(Problem::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Unprocessable Entity",
                    "Idempotency-Key reused with a different request body",
                )
                .with_type("/errors/idempotency-conflict"));
            }
            Ok(Some((status as u16, body)))
        }
    }
}

/// Store a response for future replays. Bodies are capped at 1 MB.
pub async fn idempotency_store(
    pool: &PgPool,
    key: &str,
    token_id: Uuid,
    fingerprint: &str,
    status: u16,
    body: &str,
) -> Result<(), Problem> {
    let capped: &str = if body.len() > 1_000_000 {
        // Large responses replay only the status; clients re-GET.
        "{\"replayed\":true,\"note\":\"body too large to replay\"}"
    } else {
        body
    };
    sqlx::query(
        "INSERT INTO api_idempotency
            (key, token_id, request_fingerprint, response_status, response_body)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (key, token_id) DO NOTHING",
    )
    .bind(key)
    .bind(token_id)
    .bind(fingerprint)
    .bind(i32::from(status))
    .bind(capped)
    .execute(pool)
    .await
    .map_err(db_problem)?;
    Ok(())
}

/// SHA-256 hex of the canonical request body — the fingerprint.
pub fn fingerprint(body_json: &serde_json::Value) -> String {
    let canonical = serde_json::to_string(body_json).unwrap_or_default();
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(digest)
}

pub fn db_problem(e: sqlx::Error) -> Problem {
    Problem::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error",
        e.to_string(),
    )
}
