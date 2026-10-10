//! `/api/v1/roles` — admin only (403 for everyone else).

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde_json::json;

use crate::auth::AdminUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::{Json, Path};
use crate::models::role::{Role, RoleInput};
use crate::models::user::User;
use crate::response::{created, ok};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/roles", get(index).post(store))
        .route("/roles/{id}", get(show).patch(update).delete(destroy))
        .route("/roles/{id}/users", get(users))
}

/// `GET /roles` → `data.roles`
async fn index(State(state): State<AppState>, _admin: AdminUser) -> ApiResult {
    let roles = Role::all(&state.db).await?;
    Ok(ok("Roles fetched successfully", json!({ "roles": roles })))
}

/// `POST /roles {name}` → 201 `data.role`
async fn store(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(input): Json<RoleInput>,
) -> ApiResult {
    let name = required_name(&input)?;
    let role = Role::create(&state.db, name).await?;
    Ok(created(
        "Role created successfully",
        json!({ "role": role }),
    ))
}

/// `GET /roles/:id` → `data.role`
async fn show(State(state): State<AppState>, _admin: AdminUser, Path(id): Path<i64>) -> ApiResult {
    let role = find(&state, id).await?;
    Ok(ok("Role fetched successfully", json!({ "role": role })))
}

/// `PATCH /roles/:id {name}` → `data.role`
async fn update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(input): Json<RoleInput>,
) -> ApiResult {
    let name = required_name(&input)?;
    find(&state, id).await?;
    let role = Role::update(&state.db, id, name).await?;
    Ok(ok("Role updated successfully", json!({ "role": role })))
}

/// `DELETE /roles/:id` → `data.role`
async fn destroy(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> ApiResult {
    let role = find(&state, id).await?;
    Role::delete(&state.db, id).await?;
    Ok(ok("Role deleted successfully", json!({ "role": role })))
}

/// `GET /roles/:id/users` → `data.users`
async fn users(State(state): State<AppState>, _admin: AdminUser, Path(id): Path<i64>) -> ApiResult {
    find(&state, id).await?;
    let users = User::with_role(&state.db, id).await?;
    Ok(ok("Users fetched successfully", json!({ "users": users })))
}

async fn find(state: &AppState, id: i64) -> Result<Role, ApiError> {
    Role::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Role not found"))
}

fn required_name(input: &RoleInput) -> Result<&str, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Name is required"));
    }
    Ok(name)
}
