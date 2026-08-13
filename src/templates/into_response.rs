//! Local adapter to render Askama templates as Axum responses.

use askama::Template;
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// Render an Askama template as a full HTML response.
pub fn render_response<T: Template>(tmpl: T) -> Response {
    match tmpl.render() {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Template error: {err}"),
        )
            .into_response(),
    }
}

/// A trivial wrapper that lets you write `Html(page).into_response()` if
/// you prefer the wrapper style.
pub struct Html<T: Template>(pub T);

impl<T: Template> IntoResponse for Html<T> {
    fn into_response(self) -> Response {
        render_response(self.0)
    }
}
