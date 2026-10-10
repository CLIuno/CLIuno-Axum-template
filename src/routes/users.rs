//! `/api/v1/users`. Listing and lookups need any Bearer token (the users page is a
//! normal-user page); `PATCH`/`DELETE /users/:id` are admin-gated (403 otherwise).

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde_json::json;

use crate::auth::{AdminUser, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::extract::{Json, Path};
use crate::models::post::Post;
use crate::models::role::Role;
use crate::models::user::{UpdateUser, User};
use crate::response::ok;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(index))
        .route(
            "/users/current",
            get(current).patch(update_current).delete(delete_current),
        )
        .route("/users/username/{username}", get(by_username))
        .route("/users/{id}", get(show).patch(update).delete(destroy))
        .route("/users/{id}/posts", get(posts))
        .route("/users/{id}/roles", get(role))
}

/// `GET /users` → `data.users`
async fn index(State(state): State<AppState>, _auth: AuthUser) -> ApiResult {
    let users = User::all(&state.db).await?;
    Ok(ok("Users fetched successfully", json!({ "users": users })))
}

/// `GET /users/current` → `data.user`
async fn current(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let user = find(&state, auth.id).await?;
    Ok(ok("User fetched successfully", json!({ "user": user })))
}

/// `PATCH /users/current {first_name?, last_name?, phone?, avatar_url?, ...}` → `data.user`
async fn update_current(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(changes): Json<UpdateUser>,
) -> ApiResult {
    validate(&changes)?;
    let user = User::update_profile(&state.db, auth.id, &changes).await?;
    Ok(ok("User updated successfully", json!({ "user": user })))
}

/// `DELETE /users/current` → `data.user` (soft delete)
async fn delete_current(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let user = User::soft_delete(&state.db, auth.id).await?;
    Ok(ok("User deleted successfully", json!({ "user": user })))
}

/// `GET /users/username/:username` → `data.user`
async fn by_username(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(username): Path<String>,
) -> ApiResult {
    let user = User::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    Ok(ok("User fetched successfully", json!({ "user": user })))
}

/// `GET /users/:id` → `data.user`
async fn show(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let user = find(&state, id).await?;
    Ok(ok("User fetched successfully", json!({ "user": user })))
}

/// `PATCH /users/:id` (admin) → `data.user`
async fn update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(changes): Json<UpdateUser>,
) -> ApiResult {
    validate(&changes)?;
    find(&state, id).await?;
    let user = User::update_profile(&state.db, id, &changes).await?;
    Ok(ok("User updated successfully", json!({ "user": user })))
}

/// `DELETE /users/:id` (admin) → `data.user` (soft delete)
async fn destroy(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> ApiResult {
    find(&state, id).await?;
    let user = User::soft_delete(&state.db, id).await?;
    Ok(ok("User deleted successfully", json!({ "user": user })))
}

/// `GET /users/:id/posts` → `data.posts`
async fn posts(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    find(&state, id).await?;
    let posts = Post::with_details(&state.db, Post::for_user(&state.db, id).await?).await?;
    Ok(ok("Posts fetched successfully", json!({ "posts": posts })))
}

/// `GET /users/:id/roles` → `data.role`
async fn role(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    find(&state, id).await?;
    let role = Role::of_user(&state.db, id).await?;
    Ok(ok("Role fetched successfully", json!({ "role": role })))
}

async fn find(state: &AppState, id: i64) -> Result<User, ApiError> {
    User::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))
}

fn validate(changes: &UpdateUser) -> Result<(), ApiError> {
    let blank = |value: &Option<String>| value.as_deref().is_some_and(|v| v.trim().is_empty());
    if blank(&changes.username) || blank(&changes.email) {
        return Err(ApiError::bad_request("Username and email cannot be empty"));
    }
    if changes
        .email
        .as_deref()
        .is_some_and(|email| !email.contains('@'))
    {
        return Err(ApiError::bad_request("A valid email is required"));
    }
    Ok(())
}
