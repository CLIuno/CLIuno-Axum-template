//! `/api/v1` routes — one module per resource, each exposing `routes()`.
//! A new resource adds `pub mod <name>;` here and `.merge(<name>::routes())` in [`api`].

pub mod auth;
pub mod comments;
pub mod follows;
pub mod posts;
pub mod roles;
pub mod todos;
pub mod uploads;
pub mod users;

use axum::Router;

use crate::state::AppState;

/// Every route served under `/api/v1`.
pub fn api() -> Router<AppState> {
    Router::new()
        .merge(auth::routes())
        .merge(users::routes())
        .merge(todos::routes())
        .merge(posts::routes())
        .merge(comments::routes())
        .merge(follows::routes())
        .merge(roles::routes())
        .merge(uploads::routes())
}
