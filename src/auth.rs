//! Authentication: JWT access/refresh tokens, bcrypt passwords, one-time tokens, and
//! the [`AuthUser`] / [`AdminUser`] extractors that guard routes.
//!
//! A handler that takes `auth: AuthUser` only runs for a valid `Authorization: Bearer
//! <access token>` of an existing, non-deleted user; `AdminUser` additionally requires
//! the `admin` role (403 otherwise).

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, get_current_timestamp,
};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::ApiError;
use crate::models::Owned;
use crate::models::user::User;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Access,
    Refresh,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// The user id.
    pub sub: String,
    pub kind: TokenKind,
    pub iat: u64,
    pub exp: u64,
}

fn signing_secret(config: &Config, kind: TokenKind) -> &[u8] {
    match kind {
        TokenKind::Access => config.jwt_secret.as_bytes(),
        TokenKind::Refresh => config.refresh_jwt_secret.as_bytes(),
    }
}

/// Signs an HS256 token for `user_id`; access and refresh use separate secrets.
pub fn issue_token(config: &Config, user_id: i64, kind: TokenKind) -> Result<String, ApiError> {
    let ttl = match kind {
        TokenKind::Access => config.access_token_ttl,
        TokenKind::Refresh => config.refresh_token_ttl,
    };
    let now = get_current_timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        kind,
        iat: now,
        exp: now + ttl,
    };
    let key = EncodingKey::from_secret(signing_secret(config, kind));
    encode(&Header::new(Algorithm::HS256), &claims, &key)
        .map_err(|err| ApiError::internal(format!("could not sign token: {err}")))
}

/// `(access token, refresh token)` for a successful login or refresh.
pub fn issue_token_pair(config: &Config, user_id: i64) -> Result<(String, String), ApiError> {
    Ok((
        issue_token(config, user_id, TokenKind::Access)?,
        issue_token(config, user_id, TokenKind::Refresh)?,
    ))
}

/// Returns the user id of a valid, unexpired token of the expected kind.
pub fn verify_token(config: &Config, token: &str, kind: TokenKind) -> Result<i64, ApiError> {
    let invalid = || ApiError::unauthorized("Invalid or expired token");
    let key = DecodingKey::from_secret(signing_secret(config, kind));
    let data =
        decode::<Claims>(token, &key, &Validation::new(Algorithm::HS256)).map_err(|_| invalid())?;
    if data.claims.kind != kind {
        return Err(invalid());
    }
    data.claims.sub.parse().map_err(|_| invalid())
}

/// The token from an `Authorization: Bearer <token>` header, if any.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// bcrypt on the blocking pool so hashing never stalls the async runtime.
pub async fn hash_password(password: &str, cost: u32) -> Result<String, ApiError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || bcrypt::hash(password, cost))
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?
        .map_err(|err| ApiError::internal(format!("could not hash password: {err}")))
}

pub async fn verify_password(password: &str, hash: &str) -> Result<bool, ApiError> {
    let (password, hash) = (password.to_owned(), hash.to_owned());
    tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash).unwrap_or(false))
        .await
        .map_err(|err| ApiError::internal(err.to_string()))
}

/// A random single-use token for reset-password / verify-email links.
pub fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// The authenticated user (from the Bearer access token).
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub id: i64,
}

impl AuthUser {
    /// 403 unless the authenticated user owns `item` (its `user_id`).
    pub fn ensure_owns<T: Owned>(&self, item: &T) -> Result<(), ApiError> {
        if item.owner_id() == self.id {
            Ok(())
        } else {
            Err(ApiError::forbidden(
                "Forbidden: you can only change your own resources",
            ))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = bearer_token(&parts.headers)
            .ok_or_else(|| ApiError::unauthorized("Unauthorized: missing bearer token"))?;
        let id = verify_token(&state.config, token, TokenKind::Access)?;
        if !User::is_active(&state.db, id).await? {
            return Err(ApiError::unauthorized("Unauthorized: account not found"));
        }
        Ok(Self { id })
    }
}

/// An authenticated user holding the `admin` role.
#[derive(Debug, Clone, Copy)]
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !User::is_admin(&state.db, user.id).await? {
            return Err(ApiError::forbidden("Forbidden: admin role required"));
        }
        Ok(Self(user))
    }
}
