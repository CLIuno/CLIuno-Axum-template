//! Extractors whose rejections use the contract's error envelope instead of axum's
//! plain-text defaults. Handlers import these instead of `axum::{Json, extract::Path}`.

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// A JSON request body. Lenient on purpose: the content type is not enforced and an
/// empty body reads as `{}`, so body-less POSTs from any frontend never 400/415.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|err| ApiError::from_status(err.status(), err.body_text()))?;
        let body: &[u8] = if bytes.iter().all(u8::is_ascii_whitespace) {
            b"{}"
        } else {
            &bytes
        };
        serde_json::from_slice(body)
            .map(Json)
            .map_err(|err| ApiError::bad_request(format!("Invalid request body: {err}")))
    }
}

/// Typed path parameters (`Path(id): Path<i64>`). A segment that doesn't parse —
/// `/todos/abc` — can't name an existing row, so it answers 404.
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(value)| Path(value))
            .map_err(|_| ApiError::not_found("Resource not found"))
    }
}
