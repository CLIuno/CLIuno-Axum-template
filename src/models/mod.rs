//! Database rows and their queries — one file per table. Rows derive `FromRow`
//! (read with sqlx's runtime `query_as`) and `Serialize` (snake_case JSON keys).

pub mod comment;
pub mod follow;
pub mod post;
pub mod role;
pub mod todo;
pub mod user;

use serde::Serialize;
use sqlx::SqlitePool;

use self::user::User;

/// A row that belongs to a user through its `user_id` column.
pub trait Owned {
    fn owner_id(&self) -> i64;
}

/// A row serialized with its owner embedded as `user`: frontends render
/// `todo.user.username`, `post.user.avatar_url`, `comment.user.first_name`, ...
#[derive(Debug, Serialize)]
pub struct WithOwner<T> {
    #[serde(flatten)]
    pub item: T,
    pub user: Option<User>,
}

/// Loads the owner of one row.
pub async fn with_owner<T: Owned>(db: &SqlitePool, item: T) -> sqlx::Result<WithOwner<T>> {
    let user = User::find_any(db, item.owner_id()).await?;
    Ok(WithOwner { item, user })
}

/// Loads the owners of many rows with a single query.
pub async fn with_owners<T: Owned>(
    db: &SqlitePool,
    items: Vec<T>,
) -> sqlx::Result<Vec<WithOwner<T>>> {
    let users = User::find_many(db, items.iter().map(Owned::owner_id)).await?;
    Ok(items
        .into_iter()
        .map(|item| {
            let user = users.get(&item.owner_id()).cloned();
            WithOwner { item, user }
        })
        .collect())
}
