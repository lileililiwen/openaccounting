//! RFC 7807 problem-details JSON for API errors
//! (`a1-rest-api`).
//!
//! Every 4xx / 5xx response from `/api/v1/...` carries a body
//! shaped like:
//!
//! ```json
//! {
//!   "type":   "/errors/unauthorized",
//!   "title":  "Unauthorized",
//!   "status": 401,
//!   "detail": "Missing or invalid Authorization: Bearer oa_live_… header",
//!   "instance": "/api/v1/ledgers"
//! }
//! ```

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "type")]
    pub ty: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

impl Problem {
    pub fn new(status: StatusCode, title: &str, detail: impl Into<String>) -> Self {
        Self {
            ty: format!("/errors/{}", title.to_ascii_lowercase().replace(' ', "-")),
            title: title.into(),
            status: status.as_u16(),
            detail: detail.into(),
            instance: None,
        }
    }

    pub fn with_type(mut self, ty: &str) -> Self {
        self.ty = ty.into();
        self
    }

    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Attach a header to the response (e.g. `Retry-After`).
    pub fn with_header(
        self,
        name: axum::http::HeaderName,
        value: impl Into<String>,
    ) -> ProblemWithHeaders {
        ProblemWithHeaders {
            problem: self,
            headers: vec![(name, value.into())],
        }
    }
}

/// A [`Problem`] plus extra raw headers, still RFC 7807 on the body.
pub struct ProblemWithHeaders {
    problem: Problem,
    headers: Vec<(axum::http::HeaderName, String)>,
}

impl IntoResponse for ProblemWithHeaders {
    fn into_response(self) -> Response {
        let mut resp = self.problem.into_response();
        for (name, value) in self.headers {
            if let Ok(v) = axum::http::HeaderValue::from_str(&value) {
                resp.headers_mut().insert(name, v);
            }
        }
        resp
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = (status, Json(self)).into_response();
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        resp
    }
}
