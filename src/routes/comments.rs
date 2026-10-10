//! `/api/v1/posts/:id/comments` — comments nest under their post; only the
//! comment's author may change or delete it (403).

use axum::Router;
use axum::extract::State;
use axum::routing::{get, patch};
use serde_json::json;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::{Json, Path};
use crate::models::comment::{Comment, CommentInput};
use crate::models::post::Post;
use crate::models::{with_owner, with_owners};
use crate::response::{created, ok};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/posts/{id}/comments", get(index).post(store))
        .route(
            "/posts/{id}/comments/{comment_id}",
            patch(update).delete(destroy),
        )
}

/// `GET /posts/:id/comments` → `data.comments`
async fn index(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(post_id): Path<i64>,
) -> ApiResult {
    ensure_post(&state, post_id).await?;
    let comments = with_owners(&state.db, Comment::for_post(&state.db, post_id).await?).await?;
    Ok(ok(
        "Comments fetched successfully",
        json!({ "comments": comments }),
    ))
}

/// `POST /posts/:id/comments {content}` → 201 `data.comment`
async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(post_id): Path<i64>,
    Json(input): Json<CommentInput>,
) -> ApiResult {
    if input.content.trim().is_empty() {
        return Err(ApiError::bad_request("Content is required"));
    }
    ensure_post(&state, post_id).await?;
    let comment = Comment::create(&state.db, post_id, auth.id, &input.content).await?;
    let comment = with_owner(&state.db, comment).await?;
    Ok(created(
        "Comment created successfully",
        json!({ "comment": comment }),
    ))
}

/// `PATCH /posts/:id/comments/:commentId {content}` → `data.comment`
async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((post_id, id)): Path<(i64, i64)>,
    Json(input): Json<CommentInput>,
) -> ApiResult {
    if input.content.trim().is_empty() {
        return Err(ApiError::bad_request("Content is required"));
    }
    auth.ensure_owns(&find(&state, post_id, id).await?)?;
    let comment = Comment::update(&state.db, id, &input.content).await?;
    let comment = with_owner(&state.db, comment).await?;
    Ok(ok(
        "Comment updated successfully",
        json!({ "comment": comment }),
    ))
}

/// `DELETE /posts/:id/comments/:commentId` → `data.comment`
async fn destroy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((post_id, id)): Path<(i64, i64)>,
) -> ApiResult {
    let comment = find(&state, post_id, id).await?;
    auth.ensure_owns(&comment)?;
    Comment::delete(&state.db, id).await?;
    Ok(ok(
        "Comment deleted successfully",
        json!({ "comment": comment }),
    ))
}

async fn ensure_post(state: &AppState, post_id: i64) -> Result<(), ApiError> {
    Post::find(&state.db, post_id)
        .await?
        .map(|_| ())
        .ok_or_else(|| ApiError::not_found("Post not found"))
}

async fn find(state: &AppState, post_id: i64, id: i64) -> Result<Comment, ApiError> {
    Comment::find_in_post(&state.db, post_id, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Comment not found"))
}
