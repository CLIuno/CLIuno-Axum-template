use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

/// A row of `users`. Secrets (password hash, one-time tokens, OTP secret) are
/// never serialized; every serialized user carries `avatar_url` (nullable).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub role_id: Option<i64>,
    pub is_verified: bool,
    pub is_otp_enabled: bool,
    #[serde(skip_serializing)]
    pub is_deleted: bool,
    #[serde(skip_serializing)]
    pub password: String,
    #[serde(skip_serializing)]
    pub reset_token: Option<String>,
    #[serde(skip_serializing)]
    pub verify_token: Option<String>,
    #[serde(skip_serializing)]
    pub otp_base32: Option<String>,
    #[serde(skip_serializing)]
    pub otp_auth_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Everything registration stores.
#[derive(Debug)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub email: &'a str,
    pub phone: Option<&'a str>,
    pub password_hash: &'a str,
    pub role_id: i64,
    pub verify_token: &'a str,
}

/// `PATCH /users/current` and `PATCH /users/:id` — absent keys stay unchanged.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UpdateUser {
    #[serde(alias = "firstName")]
    pub first_name: Option<String>,
    #[serde(alias = "lastName")]
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    #[serde(alias = "avatarUrl")]
    pub avatar_url: Option<String>,
}

impl User {
    /// Every active user.
    pub async fn all(db: &SqlitePool) -> sqlx::Result<Vec<User>> {
        sqlx::query_as("SELECT * FROM users WHERE is_deleted = 0 ORDER BY id")
            .fetch_all(db)
            .await
    }

    /// An active user by id.
    pub async fn find(db: &SqlitePool, id: i64) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE id = ? AND is_deleted = 0")
            .bind(id)
            .fetch_optional(db)
            .await
    }

    /// A user by id, soft-deleted or not (owners of existing rows).
    pub async fn find_any(db: &SqlitePool, id: i64) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
    }

    /// Users by id in one query, keyed by id.
    pub async fn find_many(
        db: &SqlitePool,
        ids: impl IntoIterator<Item = i64>,
    ) -> sqlx::Result<HashMap<i64, User>> {
        let ids: BTreeSet<i64> = ids.into_iter().collect();
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new("SELECT * FROM users WHERE id IN (");
        let mut list = query.separated(", ");
        for id in ids {
            list.push_bind(id);
        }
        query.push(")");
        let users: Vec<User> = query.build_query_as().fetch_all(db).await?;
        Ok(users.into_iter().map(|user| (user.id, user)).collect())
    }

    pub async fn find_by_username(db: &SqlitePool, username: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE username = ? AND is_deleted = 0")
            .bind(username)
            .fetch_optional(db)
            .await
    }

    pub async fn find_by_email(db: &SqlitePool, email: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE email = ? AND is_deleted = 0")
            .bind(email)
            .fetch_optional(db)
            .await
    }

    /// Login accepts either the username or the email.
    pub async fn find_by_login(db: &SqlitePool, login: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as(
            "SELECT * FROM users WHERE (username = ? OR email = LOWER(?)) AND is_deleted = 0",
        )
        .bind(login)
        .bind(login)
        .fetch_optional(db)
        .await
    }

    pub async fn find_by_reset_token(db: &SqlitePool, token: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE reset_token = ? AND is_deleted = 0")
            .bind(token)
            .fetch_optional(db)
            .await
    }

    pub async fn find_by_verify_token(db: &SqlitePool, token: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE verify_token = ? AND is_deleted = 0")
            .bind(token)
            .fetch_optional(db)
            .await
    }

    /// Active users holding a role.
    pub async fn with_role(db: &SqlitePool, role_id: i64) -> sqlx::Result<Vec<User>> {
        sqlx::query_as("SELECT * FROM users WHERE role_id = ? AND is_deleted = 0 ORDER BY id")
            .bind(role_id)
            .fetch_all(db)
            .await
    }

    pub async fn username_or_email_taken(
        db: &SqlitePool,
        username: &str,
        email: &str,
    ) -> sqlx::Result<bool> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = ? OR email = ?)")
            .bind(username)
            .bind(email)
            .fetch_one(db)
            .await
    }

    pub async fn create(db: &SqlitePool, new: NewUser<'_>) -> sqlx::Result<User> {
        sqlx::query_as(
            "INSERT INTO users \
             (username, first_name, last_name, email, phone, password, role_id, verify_token) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
        )
        .bind(new.username)
        .bind(new.first_name)
        .bind(new.last_name)
        .bind(new.email)
        .bind(new.phone)
        .bind(new.password_hash)
        .bind(new.role_id)
        .bind(new.verify_token)
        .fetch_one(db)
        .await
    }

    pub async fn update_profile(
        db: &SqlitePool,
        id: i64,
        changes: &UpdateUser,
    ) -> sqlx::Result<User> {
        sqlx::query_as(
            "UPDATE users SET \
             first_name = COALESCE(?, first_name), last_name = COALESCE(?, last_name), \
             username = COALESCE(?, username), email = COALESCE(?, email), \
             phone = COALESCE(?, phone), avatar_url = COALESCE(?, avatar_url), \
             updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(&changes.first_name)
        .bind(&changes.last_name)
        .bind(&changes.username)
        .bind(&changes.email)
        .bind(&changes.phone)
        .bind(&changes.avatar_url)
        .bind(id)
        .fetch_one(db)
        .await
    }

    /// Stores a new password hash and burns any pending reset token.
    pub async fn update_password(db: &SqlitePool, id: i64, hash: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE users SET password = ?, reset_token = NULL, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(hash)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_reset_token(db: &SqlitePool, id: i64, token: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE users SET reset_token = ? WHERE id = ?")
            .bind(token)
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn set_verify_token(db: &SqlitePool, id: i64, token: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE users SET verify_token = ? WHERE id = ?")
            .bind(token)
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn mark_verified(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE users SET is_verified = 1, verify_token = NULL, \
             updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Stores a fresh OTP secret; OTP stays disabled until a code is verified.
    pub async fn set_otp_secret(
        db: &SqlitePool,
        id: i64,
        base32: &str,
        auth_url: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE users SET otp_base32 = ?, otp_auth_url = ?, is_otp_enabled = 0 WHERE id = ?",
        )
        .bind(base32)
        .bind(auth_url)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn enable_otp(db: &SqlitePool, id: i64) -> sqlx::Result<User> {
        sqlx::query_as("UPDATE users SET is_otp_enabled = 1 WHERE id = ? RETURNING *")
            .bind(id)
            .fetch_one(db)
            .await
    }

    pub async fn disable_otp(db: &SqlitePool, id: i64) -> sqlx::Result<User> {
        sqlx::query_as(
            "UPDATE users SET is_otp_enabled = 0, otp_base32 = NULL, otp_auth_url = NULL \
             WHERE id = ? RETURNING *",
        )
        .bind(id)
        .fetch_one(db)
        .await
    }

    /// Soft delete: the row (and everything it owns) stays, the account disappears.
    pub async fn soft_delete(db: &SqlitePool, id: i64) -> sqlx::Result<User> {
        sqlx::query_as(
            "UPDATE users SET is_deleted = 1, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ? RETURNING *",
        )
        .bind(id)
        .fetch_one(db)
        .await
    }

    pub async fn is_active(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE id = ? AND is_deleted = 0)")
            .bind(id)
            .fetch_one(db)
            .await
    }

    pub async fn is_admin(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users u JOIN roles r ON r.id = u.role_id \
             WHERE u.id = ? AND r.name = 'admin')",
        )
        .bind(id)
        .fetch_one(db)
        .await
    }
}
