//! Public REST API (`a1-rest-api`).
//!
//! Mounted at `/api/v1/...`. Bearer-token auth via the
//! `Authorization: Bearer oa_live_…` header. Errors return
//! RFC 7807 problem-details JSON. `POST` endpoints accept an
//! `Idempotency-Key` header for safe retries.

pub mod accounts;
pub mod ledgers;
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

    let user_id = match api_token::verify_token(&state.pool, header).await {
        Some(id) => id,
        None => {
            return problem::Problem::new(
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "Missing or invalid Authorization: Bearer oa_live_… header",
            )
            .with_type("/errors/unauthorized")
            .into_response();
        }
    };

    let mut request = request;
    request.extensions_mut().insert(ApiUser(user_id));

    next.run(request).await
}

// `Backend` is referenced so the module compiles when unused
// imports warnings are on.
#[allow(dead_code, clippy::diverging_sub_expression)]
const _: fn() = || {
    let _: Backend = unreachable!();
};
