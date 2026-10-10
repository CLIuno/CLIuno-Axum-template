//! `/api/v1/posts` — posts carry their author (`user`) and `comments`.
//! Creates attach the authenticated author; only the author may change or delete (403).

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde_json::json;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::{Json, Path};
use crate::models::post::{CreatePost, Post, UpdatePost};
use crate::models::user::User;
use crate::response::{created, ok};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts", get(index).post(store))
        .route("/posts/current-user", get(current_user))
        .route("/posts/{id}", get(show).patch(update).delete(destroy))
        .route("/posts/{id}/user", get(author))
}

/// `GET /posts` → `data.posts`
async fn index(State(state): State<AppState>, _auth: AuthUser) -> ApiResult {
    let posts = Post::with_details(&state.db, Post::all(&state.db).await?).await?;
    Ok(ok("Posts fetched successfully", json!({ "posts": posts })))
}

/// `GET /posts/current-user` → `data.posts`
async fn current_user(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let posts = Post::for_user(&state.db, auth.id).await?;
    let posts = Post::with_details(&state.db, posts).await?;
    Ok(ok("Posts fetched successfully", json!({ "posts": posts })))
}

/// `GET /posts/:id` → `data.post`
async fn show(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let post = find(&state, id).await?.into_details(&state.db).await?;
    Ok(ok("Post fetched successfully", json!({ "post": post })))
}

/// `POST /posts {title, content, image_url?}` → 201 `data.post`
async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreatePost>,
) -> ApiResult {
    if input.title.trim().is_empty() {
        return Err(ApiError::bad_request("Title is required"));
    }
    let post = Post::create(&state.db, auth.id, &input).await?;
    let post = post.into_details(&state.db).await?;
    Ok(created(
        "Post created successfully",
        json!({ "post": post }),
    ))
}

/// `PATCH /posts/:id {title?, content?, image_url?}` → `data.post`
async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(input): Json<UpdatePost>,
) -> ApiResult {
    if input
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(ApiError::bad_request("Title cannot be empty"));
    }
    auth.ensure_owns(&find(&state, id).await?)?;
    let post = Post::update(&state.db, id, &input).await?;
    let post = post.into_details(&state.db).await?;
    Ok(ok("Post updated successfully", json!({ "post": post })))
}

/// `DELETE /posts/:id` → `data.post` (the deleted post, comments included)
async fn destroy(State(state): State<AppState>, auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let post = find(&state, id).await?;
    auth.ensure_owns(&post)?;
    let post = post.into_details(&state.db).await?;
    Post::delete(&state.db, id).await?;
    Ok(ok("Post deleted successfully", json!({ "post": post })))
}

/// `GET /posts/:id/user` → `data.user` (the author)
async fn author(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let post = find(&state, id).await?;
    let user = User::find_any(&state.db, post.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    Ok(ok("User fetched successfully", json!({ "user": user })))
}

async fn find(state: &AppState, id: i64) -> Result<Post, ApiError> {
    Post::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Post not found"))
}
