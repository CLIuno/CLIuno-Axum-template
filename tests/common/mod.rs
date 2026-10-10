//! Shared harness: the real router over a fresh in-memory database, driven with
//! `tower::ServiceExt::oneshot` (no sockets).

// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use cliuno_axum_template::{app, config::Config, db, state::AppState};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

pub const PASSWORD: &str = "Secret#Pass123";

pub struct TestApp {
    pub router: Router,
    pub db: SqlitePool,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub body: Value,
}

impl TestResponse {
    /// `body["data"][key]` — what the frontends destructure.
    pub fn data(&self, key: &str) -> &Value {
        &self.body["data"][key]
    }
}

impl TestApp {
    /// A fresh app over an empty, migrated database (the fresh-clone state).
    pub async fn new() -> Self {
        let db = db::connect_in_memory().await.expect("in-memory sqlite");
        db::migrate(&db).await.expect("migrations apply");
        let router = app::router(AppState::new(db.clone(), Arc::new(Config::for_tests())));
        Self { router, db }
    }

    pub async fn request(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("valid request");
        self.send(request).await
    }

    pub async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        TestResponse { status, body }
    }

    /// Registers `username` with the payload every frontend sends.
    pub async fn register(&self, username: &str) -> TestResponse {
        self.request(
            "POST",
            "/api/v1/auth/register",
            None,
            Some(json!({
                "first_name": "Test",
                "last_name": "User",
                "username": username,
                "email": format!("{username}@example.com"),
                "phone": "+15550000000",
                "password": PASSWORD,
                "password_confirmation": PASSWORD,
            })),
        )
        .await
    }

    pub async fn login(&self, username: &str, password: &str) -> TestResponse {
        self.request(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "usernameOrEmail": username, "password": password })),
        )
        .await
    }

    /// Registers + logs in; returns the access token.
    pub async fn token_for(&self, username: &str) -> String {
        assert_eq!(self.register(username).await.status, StatusCode::CREATED);
        let login = self.login(username, PASSWORD).await;
        assert_eq!(login.status, StatusCode::OK);
        login.data("token").as_str().expect("data.token").to_owned()
    }
}
