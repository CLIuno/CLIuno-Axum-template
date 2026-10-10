//! `/api/v1/auth` + the user/role rules that hang off it.

mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp};
use serde_json::json;
use totp_rs::{Builder, Secret};

#[tokio::test]
async fn register_works_on_an_empty_database() {
    // Fresh-clone rule: no seed step, the `user` role is created on first use.
    let app = TestApp::new().await;
    let roles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roles")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(roles, 0);

    let registered = app.register("first").await;
    assert_eq!(registered.status, StatusCode::CREATED);
    let user = registered.data("user");
    assert_eq!(user["username"], "first");
    assert!(
        user.get("avatar_url").is_some(),
        "avatar_url is always present"
    );
    assert!(
        user.get("password").is_none(),
        "secrets are never serialized"
    );

    let token = app
        .login("first", PASSWORD)
        .await
        .data("token")
        .as_str()
        .unwrap()
        .to_owned();
    let id = user["id"].as_i64().unwrap();
    let role = app
        .request(
            "GET",
            &format!("/api/v1/users/{id}/roles"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(role.data("role")["name"], "user");

    assert_eq!(app.register("second").await.status, StatusCode::CREATED);
    let duplicate = app.register("first").await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_returns_tokens_and_user_and_refresh_rotates() {
    let app = TestApp::new().await;
    app.register("carol").await;

    let wrong = app.login("carol", "wrong-password").await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);

    let login = app.login("carol@example.com", PASSWORD).await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.data("user")["username"], "carol");
    let token = login.data("token").as_str().unwrap().to_owned();
    let refresh = login.data("refreshToken").as_str().unwrap().to_owned();

    let check = app
        .request(
            "POST",
            "/api/v1/auth/check-token",
            Some(&token),
            Some(json!({ "token": token })),
        )
        .await;
    assert_eq!(check.status, StatusCode::OK);

    // A refresh token is not an access token, and vice versa.
    let misuse = app
        .request("GET", "/api/v1/users/current", Some(&refresh), None)
        .await;
    assert_eq!(misuse.status, StatusCode::UNAUTHORIZED);

    let rotated = app
        .request(
            "POST",
            "/api/v1/auth/refresh-token",
            None,
            Some(json!({ "refreshToken": refresh })),
        )
        .await;
    assert_eq!(rotated.status, StatusCode::OK);
    let fresh = rotated.data("token").as_str().unwrap();
    assert!(rotated.data("refreshToken").is_string());
    let me = app
        .request("GET", "/api/v1/users/current", Some(fresh), None)
        .await;
    assert_eq!(me.data("user")["username"], "carol");

    let logout = app
        .request("POST", "/api/v1/auth/logout", Some(fresh), None)
        .await;
    assert_eq!(logout.status, StatusCode::OK);
}

#[tokio::test]
async fn password_change_and_reset_by_stored_token() {
    let app = TestApp::new().await;
    let token = app.token_for("dave").await;

    let bad = app
        .request(
            "POST",
            "/api/v1/auth/change-password",
            Some(&token),
            Some(json!({ "oldPassword": "nope-nope", "newPassword": "Changed#123" })),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let changed = app
        .request(
            "POST",
            "/api/v1/auth/change-password",
            Some(&token),
            Some(json!({ "oldPassword": PASSWORD, "newPassword": "Changed#123" })),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(
        app.login("dave", "Changed#123").await.status,
        StatusCode::OK
    );

    let forgot = app
        .request(
            "POST",
            "/api/v1/auth/forgot-password",
            None,
            Some(json!({ "email": "dave@example.com" })),
        )
        .await;
    assert_eq!(forgot.status, StatusCode::OK);
    let reset_token: String =
        sqlx::query_scalar("SELECT reset_token FROM users WHERE email = 'dave@example.com'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let reset = app
        .request(
            "POST",
            "/api/v1/auth/reset-password",
            None,
            Some(json!({ "password": "Reset#4567", "token": reset_token })),
        )
        .await;
    assert_eq!(reset.status, StatusCode::OK);
    assert_eq!(app.login("dave", "Reset#4567").await.status, StatusCode::OK);

    let reused = app
        .request(
            "POST",
            "/api/v1/auth/reset-password",
            None,
            Some(json!({ "password": "Again#4567", "token": reset_token })),
        )
        .await;
    assert_eq!(
        reused.status,
        StatusCode::BAD_REQUEST,
        "reset tokens are single-use"
    );
}

#[tokio::test]
async fn email_verification_by_stored_token() {
    let app = TestApp::new().await;
    let token = app.token_for("erin").await;
    let sent = app
        .request("POST", "/api/v1/auth/send-verify-email", Some(&token), None)
        .await;
    assert_eq!(sent.status, StatusCode::OK);
    let verify_token: String =
        sqlx::query_scalar("SELECT verify_token FROM users WHERE username = 'erin'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let verified = app
        .request(
            "POST",
            "/api/v1/auth/verify-email",
            None,
            Some(json!({ "token": verify_token })),
        )
        .await;
    assert_eq!(verified.status, StatusCode::OK);
    let me = app
        .request("GET", "/api/v1/users/current", Some(&token), None)
        .await;
    assert_eq!(me.data("user")["is_verified"], true);
}

#[tokio::test]
async fn otp_lifecycle_acts_on_the_authenticated_user() {
    let app = TestApp::new().await;
    let token = app.token_for("frank").await;
    let auth = Some(token.as_str());

    let generated = app
        .request("POST", "/api/v1/auth/otp/generate", auth, None)
        .await;
    assert_eq!(generated.status, StatusCode::OK);
    let secret = generated.data("secret").as_str().unwrap().to_owned();
    assert!(
        generated
            .data("otpauth_url")
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/")
    );

    let code = Builder::new()
        .with_secret(Secret::try_from_base32(&secret).unwrap())
        .build()
        .unwrap()
        .generate_current()
        .to_string();

    let wrong = app
        .request(
            "POST",
            "/api/v1/auth/otp/verify",
            auth,
            Some(json!({ "otp": "000000" })),
        )
        .await;
    // "000000" is a valid code once in a million windows; don't flake on it.
    if code != "000000" {
        assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    }
    let verified = app
        .request(
            "POST",
            "/api/v1/auth/otp/verify",
            auth,
            Some(json!({ "otp": code })),
        )
        .await;
    assert_eq!(verified.status, StatusCode::OK);
    let validated = app
        .request(
            "POST",
            "/api/v1/auth/otp/validate",
            auth,
            Some(json!({ "otp": code })),
        )
        .await;
    assert_eq!(validated.status, StatusCode::OK);
    let disabled = app
        .request("POST", "/api/v1/auth/otp/disable", auth, None)
        .await;
    assert_eq!(disabled.status, StatusCode::OK);
    assert_eq!(disabled.data("user")["is_otp_enabled"], false);
}

#[tokio::test]
async fn admin_routes_are_gated() {
    let app = TestApp::new().await;
    let user = app.token_for("grace").await;
    let target = app.register("heidi").await.data("user")["id"]
        .as_i64()
        .unwrap();

    let users = app.request("GET", "/api/v1/users", Some(&user), None).await;
    assert_eq!(
        users.status,
        StatusCode::OK,
        "listing users is not admin-only"
    );
    let delete = app
        .request(
            "DELETE",
            &format!("/api/v1/users/{target}"),
            Some(&user),
            None,
        )
        .await;
    assert_eq!(delete.status, StatusCode::FORBIDDEN);
    let roles = app.request("GET", "/api/v1/roles", Some(&user), None).await;
    assert_eq!(roles.status, StatusCode::FORBIDDEN);

    // Promote grace to admin straight in the database, as the README describes.
    sqlx::query("INSERT INTO roles (name) VALUES ('admin')")
        .execute(&app.db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') \
         WHERE username = 'grace'",
    )
    .execute(&app.db)
    .await
    .unwrap();
    let delete = app
        .request(
            "DELETE",
            &format!("/api/v1/users/{target}"),
            Some(&user),
            None,
        )
        .await;
    assert_eq!(delete.status, StatusCode::OK);
    let roles = app.request("GET", "/api/v1/roles", Some(&user), None).await;
    assert_eq!(roles.data("roles").as_array().map(Vec::len), Some(2));
}
