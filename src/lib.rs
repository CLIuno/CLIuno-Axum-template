//! The CLIuno demo app on Rust + Axum: the shared CLIuno REST contract (auth, users,
//! todos, posts + comments, follows, roles, OTP, image uploads) under `/api/v1`,
//! backed by SQLite through sqlx.
//!
//! `main.rs` only boots; everything else lives here so integration tests can build
//! the exact same [`app::router`] against an in-memory database.

pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod extract;
pub mod mail;
pub mod models;
pub mod response;
pub mod routes;
pub mod state;
