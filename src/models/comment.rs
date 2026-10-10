use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

use super::Owned;

/// A row of `comments` (nested under a post).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Comment {
    pub id: i64,
    pub post_id: i64,
    pub user_id: i64,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `POST /posts/:id/comments` and `PATCH /posts/:id/comments/:commentId`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CommentInput {
    pub content: String,
}

impl Owned for Comment {
    fn owner_id(&self) -> i64 {
        self.user_id
    }
}

impl Comment {
    pub async fn for_post(db: &SqlitePool, post_id: i64) -> sqlx::Result<Vec<Comment>> {
        sqlx::query_as("SELECT * FROM comments WHERE post_id = ? ORDER BY id")
            .bind(post_id)
            .fetch_all(db)
            .await
    }

    /// Comments of many posts in one query.
    pub async fn for_posts(
        db: &SqlitePool,
        post_ids: impl IntoIterator<Item = i64>,
    ) -> sqlx::Result<Vec<Comment>> {
        let ids: BTreeSet<i64> = post_ids.into_iter().collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new("SELECT * FROM comments WHERE post_id IN (");
        let mut list = query.separated(", ");
        for id in ids {
            list.push_bind(id);
        }
        query.push(") ORDER BY id");
        query.build_query_as().fetch_all(db).await
    }

    /// A comment, only if it belongs to the given post.
    pub async fn find_in_post(
        db: &SqlitePool,
        post_id: i64,
        id: i64,
    ) -> sqlx::Result<Option<Comment>> {
        sqlx::query_as("SELECT * FROM comments WHERE id = ? AND post_id = ?")
            .bind(id)
            .bind(post_id)
            .fetch_optional(db)
            .await
    }

    pub async fn create(
        db: &SqlitePool,
        post_id: i64,
        user_id: i64,
        content: &str,
    ) -> sqlx::Result<Comment> {
        sqlx::query_as(
            "INSERT INTO comments (post_id, user_id, content) VALUES (?, ?, ?) RETURNING *",
        )
        .bind(post_id)
        .bind(user_id)
        .bind(content)
        .fetch_one(db)
        .await
    }

    pub async fn update(db: &SqlitePool, id: i64, content: &str) -> sqlx::Result<Comment> {
        sqlx::query_as(
            "UPDATE comments SET content = ?, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(content)
        .bind(id)
        .fetch_one(db)
        .await
    }

    pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM comments WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }
}
