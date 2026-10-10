//! The `todos` table (see `migrations/*_create_todos.sql`): the row type, the
//! request bodies, and every query the todo routes need.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use super::Owned;

/// A row of `todos`, serialized with snake_case keys.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Todo {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub is_completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `POST /todos` body. Keys are snake_case; multi-word keys also accept camelCase.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CreateTodo {
    pub title: String,
    pub description: Option<String>,
}

/// `PATCH /todos/:id` body — absent keys stay unchanged.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UpdateTodo {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "isCompleted")]
    pub is_completed: Option<bool>,
}

impl Owned for Todo {
    fn owner_id(&self) -> i64 {
        self.user_id
    }
}

impl Todo {
    pub async fn all(db: &SqlitePool) -> sqlx::Result<Vec<Todo>> {
        sqlx::query_as("SELECT * FROM todos ORDER BY id DESC")
            .fetch_all(db)
            .await
    }

    pub async fn for_user(db: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<Todo>> {
        sqlx::query_as("SELECT * FROM todos WHERE user_id = ? ORDER BY id DESC")
            .bind(user_id)
            .fetch_all(db)
            .await
    }

    pub async fn find(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Todo>> {
        sqlx::query_as("SELECT * FROM todos WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
    }

    pub async fn create(db: &SqlitePool, user_id: i64, input: &CreateTodo) -> sqlx::Result<Todo> {
        sqlx::query_as(
            "INSERT INTO todos (user_id, title, description) VALUES (?, ?, ?) RETURNING *",
        )
        .bind(user_id)
        .bind(&input.title)
        .bind(&input.description)
        .fetch_one(db)
        .await
    }

    pub async fn update(db: &SqlitePool, id: i64, input: &UpdateTodo) -> sqlx::Result<Todo> {
        sqlx::query_as(
            "UPDATE todos SET \
             title = COALESCE(?, title), \
             description = COALESCE(?, description), \
             is_completed = COALESCE(?, is_completed), \
             updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.is_completed)
        .bind(id)
        .fetch_one(db)
        .await
    }

    pub async fn toggle(db: &SqlitePool, id: i64) -> sqlx::Result<Todo> {
        sqlx::query_as(
            "UPDATE todos SET is_completed = NOT is_completed, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(id)
        .fetch_one(db)
        .await
    }

    pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM todos WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }
}
