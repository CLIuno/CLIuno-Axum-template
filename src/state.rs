use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::mail::Mailer;

/// Shared by every handler through `State<AppState>`; cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub mailer: Mailer,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Arc<Config>) -> Self {
        let mailer = Mailer::new(&config.mail);
        Self { db, config, mailer }
    }
}
