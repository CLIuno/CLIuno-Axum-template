use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

/// A row of `roles`.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `POST /roles` and `PATCH /roles/:id`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RoleInput {
    pub name: String,
}

impl Role {
    /// Assigned on registration.
    pub const USER: &'static str = "user";
    /// Unlocks the admin-gated routes.
    pub const ADMIN: &'static str = "admin";

    pub async fn all(db: &SqlitePool) -> sqlx::Result<Vec<Role>> {
        sqlx::query_as("SELECT * FROM roles ORDER BY id")
            .fetch_all(db)
            .await
    }

    pub async fn find(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Role>> {
        sqlx::query_as("SELECT * FROM roles WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
    }

    /// The role a user holds, if any.
    pub async fn of_user(db: &SqlitePool, user_id: i64) -> sqlx::Result<Option<Role>> {
        sqlx::query_as("SELECT r.* FROM roles r JOIN users u ON u.role_id = r.id WHERE u.id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await
    }

    /// Fresh-clone rule: roles are created on first use, never seed-only.
    pub async fn first_or_create(db: &SqlitePool, name: &str) -> sqlx::Result<Role> {
        sqlx::query("INSERT INTO roles (name) VALUES (?) ON CONFLICT (name) DO NOTHING")
            .bind(name)
            .execute(db)
            .await?;
        sqlx::query_as("SELECT * FROM roles WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
    }

    pub async fn create(db: &SqlitePool, name: &str) -> sqlx::Result<Role> {
        sqlx::query_as("INSERT INTO roles (name) VALUES (?) RETURNING *")
            .bind(name)
            .fetch_one(db)
            .await
    }

    pub async fn update(db: &SqlitePool, id: i64, name: &str) -> sqlx::Result<Role> {
        sqlx::query_as(
            "UPDATE roles SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING *",
        )
        .bind(name)
        .bind(id)
        .fetch_one(db)
        .await
    }

    pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM roles WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }
}
