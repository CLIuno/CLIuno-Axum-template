use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use super::comment::Comment;
use super::user::User;
use super::{Owned, WithOwner, with_owners};

/// A row of `posts`. Responses always use the snake_case `image_url` key.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Post {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content: String,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `POST /posts` body; `imageUrl` is accepted as an alias of `image_url`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CreatePost {
    pub title: String,
    pub content: String,
    #[serde(alias = "imageUrl")]
    pub image_url: Option<String>,
}

/// `PATCH /posts/:id` body — absent keys stay unchanged.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UpdatePost {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(alias = "imageUrl")]
    pub image_url: Option<String>,
}

/// A post as the frontends render it: author as `user`, plus its `comments`
/// (each with its own `user`).
#[derive(Debug, Serialize)]
pub struct PostDetails {
    #[serde(flatten)]
    pub post: Post,
    pub user: Option<User>,
    pub comments: Vec<WithOwner<Comment>>,
}

impl Owned for Post {
    fn owner_id(&self) -> i64 {
        self.user_id
    }
}

impl Post {
    pub async fn all(db: &SqlitePool) -> sqlx::Result<Vec<Post>> {
        sqlx::query_as("SELECT * FROM posts ORDER BY id DESC")
            .fetch_all(db)
            .await
    }

    pub async fn for_user(db: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<Post>> {
        sqlx::query_as("SELECT * FROM posts WHERE user_id = ? ORDER BY id DESC")
            .bind(user_id)
            .fetch_all(db)
            .await
    }

    pub async fn find(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Post>> {
        sqlx::query_as("SELECT * FROM posts WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
    }

    pub async fn create(db: &SqlitePool, user_id: i64, input: &CreatePost) -> sqlx::Result<Post> {
        sqlx::query_as(
            "INSERT INTO posts (user_id, title, content, image_url) VALUES (?, ?, ?, ?) \
             RETURNING *",
        )
        .bind(user_id)
        .bind(&input.title)
        .bind(&input.content)
        .bind(&input.image_url)
        .fetch_one(db)
        .await
    }

    pub async fn update(db: &SqlitePool, id: i64, input: &UpdatePost) -> sqlx::Result<Post> {
        sqlx::query_as(
            "UPDATE posts SET \
             title = COALESCE(?, title), \
             content = COALESCE(?, content), \
             image_url = COALESCE(?, image_url), \
             updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(&input.title)
        .bind(&input.content)
        .bind(&input.image_url)
        .bind(id)
        .fetch_one(db)
        .await
    }

    /// Deletes the post; its comments go with it (`ON DELETE CASCADE`).
    pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM posts WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Attaches authors and comments to many posts with three queries in total.
    pub async fn with_details(db: &SqlitePool, posts: Vec<Post>) -> sqlx::Result<Vec<PostDetails>> {
        let comments = Comment::for_posts(db, posts.iter().map(|post| post.id)).await?;
        let mut by_post: HashMap<i64, Vec<WithOwner<Comment>>> = HashMap::new();
        for comment in with_owners(db, comments).await? {
            by_post
                .entry(comment.item.post_id)
                .or_default()
                .push(comment);
        }
        Ok(with_owners(db, posts)
            .await?
            .into_iter()
            .map(|WithOwner { item, user }| PostDetails {
                comments: by_post.remove(&item.id).unwrap_or_default(),
                post: item,
                user,
            })
            .collect())
    }

    pub async fn into_details(self, db: &SqlitePool) -> sqlx::Result<PostDetails> {
        let mut details = Self::with_details(db, vec![self]).await?;
        Ok(details.remove(0))
    }
}
