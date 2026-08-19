//! Branded 404 fallback (`ux-language-consistency`).
//!
//! Unknown routes must render the app's own error page in the UI
//! language instead of falling through to the browser's default
//! error page. API paths get a JSON 404 so clients keep working.

use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};

pub async fn not_found(uri: Uri) -> Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "not_found" })),
        )
            .into_response();
    }
    let html = askama::Template::render(&crate::templates::error::ErrorPage {
        status_code: StatusCode::NOT_FOUND.as_u16(),
        message: "Page not found.".to_string(),
    })
    .unwrap_or_else(|_| "<h1>404 — Page not found</h1>".to_string());
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}
