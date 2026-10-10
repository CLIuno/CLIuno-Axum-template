//! One error type for every handler. It renders as the contract's error envelope
//! `{ "status": "error", "message": "..." }` with a meaningful HTTP status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::response::ApiResponse;

/// What every handler returns: a success envelope or an [`ApiError`].
pub type ApiResult<T = ApiResponse> = Result<T, ApiError>;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    PayloadTooLarge(String),
    Internal(String),
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Maps a framework rejection (bad JSON, oversized body, ...) onto our envelope.
    pub fn from_status(status: StatusCode, message: impl Into<String>) -> Self {
        let message = message.into();
        match status {
            StatusCode::UNAUTHORIZED => Self::Unauthorized(message),
            StatusCode::FORBIDDEN => Self::Forbidden(message),
            StatusCode::NOT_FOUND => Self::NotFound(message),
            StatusCode::CONFLICT => Self::Conflict(message),
            StatusCode::PAYLOAD_TOO_LARGE => Self::PayloadTooLarge(message),
            status if status.is_server_error() => Self::Internal(message),
            _ => Self::BadRequest(message),
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(m)
            | Self::Unauthorized(m)
            | Self::Forbidden(m)
            | Self::NotFound(m)
            | Self::Conflict(m)
            | Self::PayloadTooLarge(m)
            | Self::Internal(m) => m,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status_code(), self.message())
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = match &self {
            // Details of internal failures go to the log, never to the client.
            Self::Internal(detail) => {
                tracing::error!("internal error: {detail}");
                "Internal server error"
            }
            other => other.message(),
        };
        let body = Json(json!({ "status": "error", "message": message }));
        (self.status_code(), body).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Self::not_found("Resource not found"),
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                Self::conflict("A record with these details already exists")
            }
            _ => Self::Internal(err.to_string()),
        }
    }
}

impl From<axum::extract::multipart::MultipartError> for ApiError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        Self::from_status(err.status(), err.body_text())
    }
}
