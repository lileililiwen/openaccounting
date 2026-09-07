//! Public REST API (`a1-rest-api`).
//!
//! Mounted at `/api/v1/...`. Bearer-token auth via the
//! `Authorization: Bearer oa_live_…` header. Errors return
//! RFC 7807 problem-details JSON. `POST` endpoints accept an
//! `Idempotency-Key` header for safe retries.

pub mod accounts;
pub mod bank_feeds;
pub mod budgets;
pub mod contacts;
pub mod documents;
pub mod helpers;
pub mod invoices;
pub mod ledgers;
pub mod payments;
pub mod problem;
pub mod reports;
pub mod transactions;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::{api_token, Backend},
    AppState,
};

/// Alias used by handlers — every API endpoint takes `State<AppState>`.
pub type ApiState = AppState;

/// The user_id injected into request extensions by the bearer
/// middleware. Handlers extract this via `Extension<ApiUser>`.
#[derive(Clone, Copy, Debug)]
pub struct ApiUser(pub Uuid);

/// How to extract `ApiUser` from request extensions in handlers.
impl<S> FromRequestParts<S> for ApiUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiUser>()
            .copied()
            .ok_or((StatusCode::UNAUTHORIZED, "missing ApiUser extension"))
    }
}

/// Build the `/api/v1` router. Auth is applied via the
/// `require_bearer` middleware on every `/api/v1/...` route,
/// scoped to that prefix only.
pub fn router(state: AppState) -> Router<AppState> {
    // Build the API routes under `/api/v1` and layer
    // `require_bearer` on that sub-tree only. Using `nest`
    // rather than `merge` keeps the middleware scoped: the
    // bearer requirement doesn't bleed into the surrounding
    // router (e.g. `/metrics`).
    let api_routes: Router<AppState> = Router::new()
        .merge(ledgers::router())
        .merge(accounts::router())
        .merge(transactions::router())
        .merge(invoices::router())
        .merge(payments::router())
        .merge(contacts::router())
        .merge(documents::router())
        .merge(budgets::router())
        .merge(bank_feeds::router())
        .merge(reports::router())
        .layer(middleware::from_fn_with_state(state, require_bearer));

    Router::new().nest("/api/v1", api_routes)
}

/// Bearer-token middleware. Rejects requests that don't carry a
/// valid `Authorization: Bearer oa_live_…` header with HTTP 401
/// + RFC 7807 problem-details JSON.
async fn require_bearer(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let Some((user_id, token_id)) = api_token::verify_token(&state.pool, header).await else {
        return problem::Problem::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "Missing or invalid Authorization: Bearer oa_live_… header",
        )
        .with_type("/errors/unauthorized")
        .into_response();
    };

    // Per-token rate limit (`api-v2-coverage`): sliding window,
    // default 120 req/min. 429 + Retry-After when exceeded.
    if let Err(retry_after) = rate_limit::check(token_id) {
        return problem::Problem::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Too Many Requests",
            "API token rate limit exceeded",
        )
        .with_type("/errors/rate-limited")
        .with_header(axum::http::header::RETRY_AFTER, retry_after.to_string())
        .into_response();
    }

    let mut request = request;
    request.extensions_mut().insert(ApiUser(user_id));
    request.extensions_mut().insert(ApiTokenId(token_id));

    next.run(request).await
}

/// The API token id injected into request extensions.
#[derive(Clone, Copy, Debug)]
pub struct ApiTokenId(pub Uuid);

impl<S> FromRequestParts<S> for ApiTokenId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiTokenId>()
            .copied()
            .ok_or((StatusCode::UNAUTHORIZED, "missing ApiTokenId extension"))
    }
}

/// In-memory per-token sliding-window rate limiter (`api-v2-coverage`).
/// Single-node semantics by design; SMB scale makes cross-node sync
/// unnecessary. Fixed-capacity map bounds memory under key churn.
pub mod rate_limit {
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    pub const DEFAULT_LIMIT_PER_MIN: u32 = 120;
    const WINDOW: Duration = Duration::from_secs(60);
    const MAX_TRACKED_TOKENS: usize = 10_000;

    struct Entry {
        hits: VecDeque<Instant>,
    }

    static STATE: once_cell::sync::Lazy<Mutex<HashMap<Uuid, Entry>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

    /// Per-token limit overrides. Production never touches this;
    /// tests use it to exercise throttling deterministically without
    /// depending on wall-clock windows.
    static TOKEN_OVERRIDES: once_cell::sync::Lazy<Mutex<HashMap<Uuid, u32>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

    /// Set a specific request-per-minute limit for one token
    /// (`None` clears). Test-support only.
    #[cfg(feature = "test-support")]
    pub fn set_token_limit(token_id: Uuid, limit: Option<u32>) {
        let mut map = TOKEN_OVERRIDES.lock().unwrap();
        match limit {
            Some(l) => {
                map.insert(token_id, l);
            }
            None => {
                map.remove(&token_id);
            }
        }
    }

    /// Returns `Ok(())` or `Err(retry_after_seconds)`.
    pub fn check(token_id: Uuid) -> Result<(), u64> {
        let limit = TOKEN_OVERRIDES
            .lock()
            .unwrap()
            .get(&token_id)
            .copied()
            .or_else(|| {
                std::env::var("API_RATE_LIMIT_PER_MIN")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
            })
            .unwrap_or(DEFAULT_LIMIT_PER_MIN);
        let now = Instant::now();
        let mut state = STATE.lock().unwrap();
        if state.len() >= MAX_TRACKED_TOKENS && !state.contains_key(&token_id) {
            // Bound memory under key churn: drop everything and give
            // existing clients a fresh window (worst case they retry).
            state.clear();
        }
        let entry = state.entry(token_id).or_insert_with(|| Entry {
            hits: VecDeque::new(),
        });
        while let Some(front) = entry.hits.front() {
            if now.duration_since(*front) >= WINDOW {
                entry.hits.pop_front();
            } else {
                break;
            }
        }
        if entry.hits.len() as u32 >= limit {
            let retry = entry
                .hits
                .front()
                .map(|t| WINDOW.saturating_sub(now.duration_since(*t)).as_secs() + 1)
                .unwrap_or(1);
            return Err(retry);
        }
        entry.hits.push_back(now);
        Ok(())
    }
}

// `Backend` is referenced so the module compiles when unused
// imports warnings are on.
#[allow(dead_code, clippy::diverging_sub_expression)]
const _: fn() = || {
    let _: Backend = unreachable!();
};
