//! `/api/v1/todos` — every route needs a Bearer token; creates attach the
//! authenticated user as owner and only the owner may change or delete (403).

use axum::Router;
use axum::extract::State;
use axum::routing::{get, patch};
use serde_json::json;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::extract::{Json, Path};
use crate::models::todo::{CreateTodo, Todo, UpdateTodo};
use crate::models::{with_owner, with_owners};
use crate::response::{created, ok};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/todos", get(index).post(store))
        .route("/todos/current-user", get(current_user))
        .route("/todos/{id}", get(show).patch(update).delete(destroy))
        .route("/todos/{id}/toggle", patch(toggle))
}

/// `GET /todos` → `data.todos`
async fn index(State(state): State<AppState>, _auth: AuthUser) -> ApiResult {
    let todos = with_owners(&state.db, Todo::all(&state.db).await?).await?;
    Ok(ok("Todos fetched successfully", json!({ "todos": todos })))
}

/// `GET /todos/current-user` → `data.todos` (the caller's own)
async fn current_user(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let todos = with_owners(&state.db, Todo::for_user(&state.db, auth.id).await?).await?;
    Ok(ok("Todos fetched successfully", json!({ "todos": todos })))
}

/// `GET /todos/:id` → `data.todo`
async fn show(State(state): State<AppState>, _auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let todo = find(&state, id).await?;
    let todo = with_owner(&state.db, todo).await?;
    Ok(ok("Todo fetched successfully", json!({ "todo": todo })))
}

/// `POST /todos {title, description?}` → 201 `data.todo`
async fn store(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateTodo>,
) -> ApiResult {
    if input.title.trim().is_empty() {
        return Err(ApiError::bad_request("Title is required"));
    }
    let todo = Todo::create(&state.db, auth.id, &input).await?;
    let todo = with_owner(&state.db, todo).await?;
    Ok(created(
        "Todo created successfully",
        json!({ "todo": todo }),
    ))
}

/// `PATCH /todos/:id {title?, description?, is_completed?}` → `data.todo`
async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(input): Json<UpdateTodo>,
) -> ApiResult {
    if input
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(ApiError::bad_request("Title cannot be empty"));
    }
    auth.ensure_owns(&find(&state, id).await?)?;
    let todo = Todo::update(&state.db, id, &input).await?;
    let todo = with_owner(&state.db, todo).await?;
    Ok(ok("Todo updated successfully", json!({ "todo": todo })))
}

/// `PATCH /todos/:id/toggle` → `data.todo`
async fn toggle(State(state): State<AppState>, auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    auth.ensure_owns(&find(&state, id).await?)?;
    let todo = Todo::toggle(&state.db, id).await?;
    let todo = with_owner(&state.db, todo).await?;
    Ok(ok("Todo toggled successfully", json!({ "todo": todo })))
}

/// `DELETE /todos/:id` → `data.todo` (the deleted row)
async fn destroy(State(state): State<AppState>, auth: AuthUser, Path(id): Path<i64>) -> ApiResult {
    let todo = find(&state, id).await?;
    auth.ensure_owns(&todo)?;
    Todo::delete(&state.db, id).await?;
    Ok(ok("Todo deleted successfully", json!({ "todo": todo })))
}

async fn find(state: &AppState, id: i64) -> Result<Todo, ApiError> {
    Todo::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Todo not found"))
}
