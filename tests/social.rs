//! Posts + nested comments, follows, and image uploads.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn posts_carry_author_and_comments() {
    let app = TestApp::new().await;
    let author = app.token_for("ivan").await;
    let reader = app.token_for("judy").await;

    let post = app
        .request(
            "POST",
            "/api/v1/posts",
            Some(&author),
            Some(json!({ "title": "Hello", "content": "World", "imageUrl": "http://x/y.png" })),
        )
        .await;
    assert_eq!(post.status, StatusCode::CREATED);
    let id = post.data("post")["id"].as_i64().unwrap();
    assert_eq!(
        post.data("post")["image_url"],
        "http://x/y.png",
        "imageUrl alias"
    );

    let comment = app
        .request(
            "POST",
            &format!("/api/v1/posts/{id}/comments"),
            Some(&reader),
            Some(json!({ "content": "Nice" })),
        )
        .await;
    assert_eq!(comment.status, StatusCode::CREATED);
    let comment_id = comment.data("comment")["id"].as_i64().unwrap();

    let shown = app
        .request("GET", &format!("/api/v1/posts/{id}"), Some(&reader), None)
        .await;
    let shown = shown.data("post");
    assert_eq!(shown["user"]["username"], "ivan");
    assert_eq!(shown["comments"][0]["user"]["username"], "judy");

    let not_yours = app
        .request(
            "PATCH",
            &format!("/api/v1/posts/{id}/comments/{comment_id}"),
            Some(&author),
            Some(json!({ "content": "edited by someone else" })),
        )
        .await;
    assert_eq!(not_yours.status, StatusCode::FORBIDDEN);
    let not_yours = app
        .request(
            "DELETE",
            &format!("/api/v1/posts/{id}"),
            Some(&reader),
            None,
        )
        .await;
    assert_eq!(not_yours.status, StatusCode::FORBIDDEN);

    let deleted = app
        .request(
            "DELETE",
            &format!("/api/v1/posts/{id}"),
            Some(&author),
            None,
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    let comments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(comments, 0, "comments cascade with their post");
}

#[tokio::test]
async fn follow_and_unfollow() {
    let app = TestApp::new().await;
    let me = app.token_for("kim").await;
    let other = app.register("lee").await.data("user")["id"]
        .as_i64()
        .unwrap();

    let follow = app
        .request(
            "POST",
            &format!("/api/v1/follows/{other}/follow"),
            Some(&me),
            None,
        )
        .await;
    assert_eq!(follow.status, StatusCode::OK);
    let followers = app
        .request(
            "GET",
            &format!("/api/v1/follows/{other}/followers"),
            Some(&me),
            None,
        )
        .await;
    assert_eq!(followers.data("followers")[0]["username"], "kim");
    let status = app
        .request(
            "GET",
            &format!("/api/v1/follows/{other}/is-following"),
            Some(&me),
            None,
        )
        .await;
    assert_eq!(status.data("isFollowing"), true);

    app.request(
        "DELETE",
        &format!("/api/v1/follows/{other}/follow"),
        Some(&me),
        None,
    )
    .await;
    let status = app
        .request(
            "GET",
            &format!("/api/v1/follows/{other}/is-following"),
            Some(&me),
            None,
        )
        .await;
    assert_eq!(status.data("isFollowing"), false);
}

#[tokio::test]
async fn image_upload_returns_an_absolute_url() {
    // Smallest valid PNG (1x1).
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC,
        0xCF, 0xC0, 0x50, 0x0F, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xA9, 0x8C, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let multipart = |token: Option<&str>, bytes: &[u8]| {
        let boundary = "cliuno-test-boundary";
        let mut body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let mut request = Request::post("/api/v1/uploads/image")
            .header(header::HOST, "api.test:3000")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            );
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        request.body(Body::from(body)).unwrap()
    };

    let app = TestApp::new().await;
    assert_eq!(
        app.send(multipart(None, PNG)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let token = app.token_for("mallory").await;

    let uploaded = app.send(multipart(Some(&token), PNG)).await;
    assert_eq!(uploaded.status, StatusCode::CREATED);
    let url = uploaded.data("url").as_str().unwrap();
    let filename = uploaded.data("filename").as_str().unwrap();
    assert!(filename.ends_with(".png"));
    assert_eq!(url, format!("http://api.test:3000/uploads/{filename}"));

    let served = app
        .send(
            Request::get(format!("/uploads/{filename}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(served.status, StatusCode::OK);

    let not_an_image = app.send(multipart(Some(&token), b"plain text")).await;
    assert_eq!(not_an_image.status, StatusCode::BAD_REQUEST);
}
