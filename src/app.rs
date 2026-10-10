//! The HTTP application: `/api/v1` routes, public `/uploads`, CORS, tracing.

use axum::Router;
use axum::routing::get;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::error::{ApiError, ApiResult};
use crate::response::ok;
use crate::routes;
use crate::state::AppState;

/// Builds the whole app. `main.rs` serves it; tests drive it with `oneshot`.
pub fn router(state: AppState) -> Router {
    // Uploaded images are public and unversioned: GET /uploads/<uuid>.<ext>.
    let uploads = ServeDir::new(&state.config.upload_dir);

    Router::new()
        .route("/", get(root))
        .nest("/api/v1", routes::api())
        .nest_service("/uploads", uploads)
        .fallback(not_found)
        // Frontends run on their own dev origins and send Bearer tokens (no cookies),
        // so any origin may call the API. Tighten this for production deployments.
        .layer(CorsLayer::permissive())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

async fn root() -> ApiResult {
    Ok(ok(
        "CLIuno Axum API",
        json!({ "api": "/api/v1", "version": env!("CARGO_PKG_VERSION") }),
    ))
}

async fn not_found() -> ApiError {
    ApiError::not_found("Route not found")
}
