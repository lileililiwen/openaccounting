use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("validation: {0}")]
    Validation(String),

    #[error("unprocessable: {0}")]
    Unprocessable(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("database: {0}")]
    Db(#[from] sqlx::Error),

    #[error("template: {0}")]
    Template(#[from] askama::Error),

    #[error("multipart: {0}")]
    Multipart(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<csv::Error> for AppError {
    fn from(e: csv::Error) -> Self {
        AppError::Internal(format!("csv: {e}"))
    }
}

impl From<crate::import::ParseError> for AppError {
    fn from(e: crate::import::ParseError) -> Self {
        AppError::Validation(e.to_string())
    }
}

impl From<axum_login::Error<crate::auth::Backend>> for AppError {
    fn from(e: axum_login::Error<crate::auth::Backend>) -> Self {
        AppError::Internal(format!("auth: {e}"))
    }
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::Validation(_) | AppError::Multipart(_) => StatusCode::BAD_REQUEST,
            AppError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Io(_) | AppError::Db(_) | AppError::Template(_) | AppError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let msg = self.to_string();

        if status.is_server_error() {
            tracing::error!(error = %msg, "request failed");
        } else {
            tracing::warn!(error = %msg, "request rejected");
        }

        if let Ok(html) = askama::Template::render(&crate::templates::error::ErrorPage {
            status_code: status.as_u16(),
            message: msg.clone(),
        }) {
            return (
                status,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                html,
            )
                .into_response();
        }

        (status, msg).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
