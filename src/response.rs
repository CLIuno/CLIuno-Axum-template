//! The success envelope: `{ "status": "success", "message": "...", "data": { ... } }`.
//!
//! Frontends destructure `data.<key>` (`data.todos`, `data.todo`, `data.user`, ...),
//! so handlers always pass a keyed object, e.g. `ok("Todo created", json!({ "todo": todo }))`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

#[derive(Debug)]
pub struct ApiResponse {
    pub status: StatusCode,
    pub message: String,
    pub data: Value,
}

/// 200 with `data`.
pub fn ok(message: impl Into<String>, data: Value) -> ApiResponse {
    ApiResponse {
        status: StatusCode::OK,
        message: message.into(),
        data,
    }
}

/// 201 with `data` — for creates.
pub fn created(message: impl Into<String>, data: Value) -> ApiResponse {
    ApiResponse {
        status: StatusCode::CREATED,
        message: message.into(),
        data,
    }
}

/// 200 with an empty `data` object — for actions that return nothing.
pub fn message(message: impl Into<String>) -> ApiResponse {
    ok(message, json!({}))
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        let body = json!({
            "status": "success",
            "message": self.message,
            "data": self.data,
        });
        (self.status, Json(body)).into_response()
    }
}
