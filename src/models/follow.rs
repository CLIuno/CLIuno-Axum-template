use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

use super::user::User;

/// A row of `follows`: `follower_id` follows `following_id`.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Follow {
    pub id: i64,
    pub follower_id: i64,
    pub following_id: i64,
    pub created_at: DateTime<Utc>,
}

impl Follow {
    /// Idempotent: following twice keeps one row.
    pub async fn create(db: &SqlitePool, follower_id: i64, following_id: i64) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO follows (follower_id, following_id) VALUES (?, ?) \
             ON CONFLICT (follower_id, following_id) DO NOTHING",
        )
        .bind(follower_id)
        .bind(following_id)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete(db: &SqlitePool, follower_id: i64, following_id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM follows WHERE follower_id = ? AND following_id = ?")
            .bind(follower_id)
            .bind(following_id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn exists(
        db: &SqlitePool,
        follower_id: i64,
        following_id: i64,
    ) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM follows WHERE follower_id = ? AND following_id = ?)",
        )
        .bind(follower_id)
        .bind(following_id)
        .fetch_one(db)
        .await
    }

    /// Active users following `user_id`.
    pub async fn followers_of(db: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<User>> {
        sqlx::query_as(
            "SELECT u.* FROM users u JOIN follows f ON f.follower_id = u.id \
             WHERE f.following_id = ? AND u.is_deleted = 0 ORDER BY f.id DESC",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
    }

    /// Active users `user_id` follows.
    pub async fn following_of(db: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<User>> {
        sqlx::query_as(
            "SELECT u.* FROM users u JOIN follows f ON f.following_id = u.id \
             WHERE f.follower_id = ? AND u.is_deleted = 0 ORDER BY f.id DESC",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
    }
}
