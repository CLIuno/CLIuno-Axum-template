//! Runtime configuration, read once from the environment (and `.env`).

use std::env;
use std::path::PathBuf;

const DEV_JWT_SECRET: &str = "cliuno-axum-development-secret-change-me";
const DEV_REFRESH_SECRET: &str = "cliuno-axum-development-refresh-secret-change-me";

#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP port (`PORT`, default 3000).
    pub port: u16,
    /// sqlx SQLite URL (`DATABASE_URL`, default `sqlite://cliuno.db`).
    pub database_url: String,
    /// Signs access tokens (`JWT_SECRET_KEY`).
    pub jwt_secret: String,
    /// Signs refresh tokens (`REFRESH_JWT_SECRET_KEY`).
    pub refresh_jwt_secret: String,
    /// Access-token lifetime in seconds (`ACCESS_TOKEN_TTL`, default 1 hour).
    pub access_token_ttl: u64,
    /// Refresh-token lifetime in seconds (`REFRESH_TOKEN_TTL`, default 7 days).
    pub refresh_token_ttl: u64,
    /// bcrypt work factor (`BCRYPT_COST`, default 10, minimum 4).
    pub bcrypt_cost: u32,
    /// Absolute origin for upload URLs (`PUBLIC_BASE_URL`); the request's Host otherwise.
    pub public_base_url: Option<String>,
    /// Where email links point (`FRONTEND_URL`).
    pub frontend_url: String,
    /// Directory uploaded images are written to and served from (`UPLOAD_DIR`).
    pub upload_dir: PathBuf,
    pub mail: MailConfig,
}

#[derive(Debug, Clone)]
pub struct MailConfig {
    /// SMTP host (`MAIL_HOST`). Unset means mails are logged instead of sent.
    pub host: Option<String>,
    /// SMTP port (`MAIL_PORT`, default 1025 — mailpit / any local sink).
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Sender address (`MAIL_FROM_ADDRESS`).
    pub from: String,
}

/// A set, non-blank environment variable.
fn var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    var(key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

impl Config {
    pub fn from_env() -> Self {
        let jwt_secret = var("JWT_SECRET_KEY").unwrap_or_else(|| {
            tracing::warn!("JWT_SECRET_KEY is not set; using the development secret");
            DEV_JWT_SECRET.to_owned()
        });
        Self {
            port: parsed("PORT", 3000),
            database_url: var("DATABASE_URL").unwrap_or_else(|| "sqlite://cliuno.db".to_owned()),
            refresh_jwt_secret: var("REFRESH_JWT_SECRET_KEY")
                .unwrap_or_else(|| DEV_REFRESH_SECRET.to_owned()),
            jwt_secret,
            access_token_ttl: parsed("ACCESS_TOKEN_TTL", 60 * 60),
            refresh_token_ttl: parsed("REFRESH_TOKEN_TTL", 7 * 24 * 60 * 60),
            bcrypt_cost: parsed("BCRYPT_COST", 10).clamp(4, 31),
            public_base_url: var("PUBLIC_BASE_URL"),
            frontend_url: var("FRONTEND_URL").unwrap_or_else(|| "http://localhost:5173".to_owned()),
            upload_dir: var("UPLOAD_DIR")
                .unwrap_or_else(|| "uploads".to_owned())
                .into(),
            mail: MailConfig {
                host: var("MAIL_HOST"),
                port: parsed("MAIL_PORT", 1025),
                username: var("MAIL_USERNAME"),
                password: var("MAIL_PASSWORD"),
                from: var("MAIL_FROM_ADDRESS")
                    .unwrap_or_else(|| "no-reply@cliuno.local".to_owned()),
            },
        }
    }

    /// Settings for the integration tests: in-memory database, the cheapest bcrypt
    /// cost, no SMTP, and a throw-away upload directory.
    pub fn for_tests() -> Self {
        Self {
            port: 0,
            database_url: "sqlite::memory:".to_owned(),
            jwt_secret: "test-secret".to_owned(),
            refresh_jwt_secret: "test-refresh-secret".to_owned(),
            access_token_ttl: 60 * 60,
            refresh_token_ttl: 7 * 24 * 60 * 60,
            bcrypt_cost: 4,
            public_base_url: None,
            frontend_url: "http://localhost:5173".to_owned(),
            upload_dir: env::temp_dir().join("cliuno-axum-test-uploads"),
            mail: MailConfig {
                host: None,
                port: 1025,
                username: None,
                password: None,
                from: "no-reply@cliuno.local".to_owned(),
            },
        }
    }
}
