//! `/api/v1/follows/:id/...` — `:id` is the target user; the Bearer user acts.

use axum::Router;
use axum::extract::State;
use axum::routing::{get, post};
use serde_json::json;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::Path;
use crate::models::follow::Follow;
use crate::models::user::User;
use crate::response::ok;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/follows/{id}/follow", post(follow).delete(unfollow))
        .route("/follows/{id}/followers", get(followers))
        .route("/follows/{id}/following", get(following))
        .route("/follows/{id}/is-following", get(is_following))
}

/// `POST /follows/:id/follow`
async fn follow(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(target): Path<i64>,
) -> ApiResult {
    if target == auth.id {
        return Err(ApiError::bad_request("You cannot follow yourself"));
    }
    ensure_user(&state, target).await?;
    Follow::create(&state.db, auth.id, target).await?;
    Ok(ok(
        "User followed successfully",
        json!({ "isFollowing": true }),
    ))
}

/// `DELETE /follows/:id/follow`
async fn unfollow(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(target): Path<i64>,
) -> ApiResult {
    Follow::delete(&state.db, auth.id, target).await?;
    Ok(ok(
        "User unfollowed successfully",
        json!({ "isFollowing": false }),
    ))
}

/// `GET /follows/:id/followers` → `data.followers`
async fn followers(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(user_id): Path<i64>,
) -> ApiResult {
    ensure_user(&state, user_id).await?;
    let followers = Follow::followers_of(&state.db, user_id).await?;
    Ok(ok(
        "Followers fetched successfully",
        json!({ "followers": followers }),
    ))
}

/// `GET /follows/:id/following` → `data.following`
async fn following(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(user_id): Path<i64>,
) -> ApiResult {
    ensure_user(&state, user_id).await?;
    let following = Follow::following_of(&state.db, user_id).await?;
    Ok(ok(
        "Following fetched successfully",
        json!({ "following": following }),
    ))
}

/// `GET /follows/:id/is-following` → `data.isFollowing` (camelCase key)
async fn is_following(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(target): Path<i64>,
) -> ApiResult {
    let is_following = Follow::exists(&state.db, auth.id, target).await?;
    Ok(ok(
        "Follow status fetched successfully",
        json!({ "isFollowing": is_following }),
    ))
}

async fn ensure_user(state: &AppState, id: i64) -> Result<(), ApiError> {
    User::find(&state.db, id)
        .await?
        .map(|_| ())
        .ok_or_else(|| ApiError::not_found("User not found"))
}
