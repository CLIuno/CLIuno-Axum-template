//! `/api/v1/todos` — the reference resource: CRUD, owner checks, auth.

mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn todos_crud_round_trip() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    let auth = Some(token.as_str());

    let created = app
        .request(
            "POST",
            "/api/v1/todos",
            auth,
            Some(json!({ "title": "Write tests", "description": "with oneshot" })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let todo = created.data("todo");
    let id = todo["id"].as_i64().expect("data.todo.id");
    assert_eq!(todo["title"], "Write tests");
    assert_eq!(todo["is_completed"], false);
    assert_eq!(todo["user"]["username"], "alice", "owner is embedded");

    let list = app.request("GET", "/api/v1/todos", auth, None).await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.data("todos").as_array().map(Vec::len), Some(1));

    let mine = app
        .request("GET", "/api/v1/todos/current-user", auth, None)
        .await;
    assert_eq!(mine.data("todos")[0]["id"], id);

    let shown = app
        .request("GET", &format!("/api/v1/todos/{id}"), auth, None)
        .await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(shown.data("todo")["description"], "with oneshot");

    let updated = app
        .request(
            "PATCH",
            &format!("/api/v1/todos/{id}"),
            auth,
            Some(json!({ "title": "Write more tests" })),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.data("todo")["title"], "Write more tests");
    assert_eq!(
        updated.data("todo")["description"],
        "with oneshot",
        "absent keys stay unchanged"
    );

    let toggled = app
        .request("PATCH", &format!("/api/v1/todos/{id}/toggle"), auth, None)
        .await;
    assert_eq!(toggled.status, StatusCode::OK);
    assert_eq!(toggled.data("todo")["is_completed"], true);

    let deleted = app
        .request("DELETE", &format!("/api/v1/todos/{id}"), auth, None)
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.data("todo")["id"], id);

    let gone = app
        .request("GET", &format!("/api/v1/todos/{id}"), auth, None)
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    assert_eq!(gone.body["status"], "error");
}

#[tokio::test]
async fn only_the_owner_can_change_a_todo() {
    let app = TestApp::new().await;
    let owner = app.token_for("owner").await;
    let other = app.token_for("intruder").await;

    let created = app
        .request(
            "POST",
            "/api/v1/todos",
            Some(&owner),
            Some(json!({ "title": "mine" })),
        )
        .await;
    let id = created.data("todo")["id"].as_i64().expect("id");
    let path = format!("/api/v1/todos/{id}");

    let patch = app
        .request("PATCH", &path, Some(&other), Some(json!({ "title": "x" })))
        .await;
    assert_eq!(patch.status, StatusCode::FORBIDDEN);
    assert_eq!(patch.body["status"], "error");
    let toggle = app
        .request("PATCH", &format!("{path}/toggle"), Some(&other), None)
        .await;
    assert_eq!(toggle.status, StatusCode::FORBIDDEN);
    let delete = app.request("DELETE", &path, Some(&other), None).await;
    assert_eq!(delete.status, StatusCode::FORBIDDEN);

    // Reading is open to any authenticated user; the owner can still delete.
    let read = app.request("GET", &path, Some(&other), None).await;
    assert_eq!(read.status, StatusCode::OK);
    let delete = app.request("DELETE", &path, Some(&owner), None).await;
    assert_eq!(delete.status, StatusCode::OK);
}

#[tokio::test]
async fn todos_require_a_token_and_a_title() {
    let app = TestApp::new().await;
    let anonymous = app.request("GET", "/api/v1/todos", None, None).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(anonymous.body["status"], "error");

    let bogus = app
        .request("GET", "/api/v1/todos", Some("not-a-jwt"), None)
        .await;
    assert_eq!(bogus.status, StatusCode::UNAUTHORIZED);

    let token = app.token_for("bob").await;
    let untitled = app
        .request(
            "POST",
            "/api/v1/todos",
            Some(&token),
            Some(json!({ "description": "no title" })),
        )
        .await;
    assert_eq!(untitled.status, StatusCode::BAD_REQUEST);
}
