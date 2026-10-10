//! `/api/v1/auth` — register/login with JWT access + refresh tokens, password change and
//! reset, email verification, and RFC 6238 OTP (SHA1, 6 digits, 30 s).
//!
//! Request keys follow the frontends: camelCase `usernameOrEmail`, `refreshToken`,
//! `oldPassword`/`newPassword`, `otp`; snake_case aliases are accepted as well.
//! One-time tokens live on the user row and are looked up by token.

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use serde::Deserialize;
use serde_json::json;
use totp_rs::{Builder, Secret, Totp};

use crate::auth::{
    AuthUser, TokenKind, bearer_token, hash_password, issue_token_pair, random_token,
    verify_password, verify_token,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::Json;
use crate::models::role::Role;
use crate::models::user::{NewUser, User};
use crate::response::{created, message, ok};
use crate::state::AppState;

const MIN_PASSWORD_LEN: usize = 8;
const OTP_ISSUER: &str = "CLIuno";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/check-token", post(check_token))
        .route("/auth/refresh-token", post(refresh_token))
        .route("/auth/change-password", post(change_password))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
        .route("/auth/send-verify-email", post(send_verify_email))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/otp/generate", post(otp_generate))
        .route("/auth/otp/verify", post(otp_verify))
        .route("/auth/otp/validate", post(otp_validate))
        .route("/auth/otp/disable", post(otp_disable))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RegisterInput {
    #[serde(alias = "firstName")]
    first_name: String,
    #[serde(alias = "lastName")]
    last_name: String,
    username: String,
    email: String,
    phone: Option<String>,
    password: String,
    #[serde(alias = "passwordConfirmation")]
    password_confirmation: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginInput {
    #[serde(rename = "usernameOrEmail", alias = "username_or_email")]
    username_or_email: String,
    password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TokenInput {
    token: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RefreshInput {
    #[serde(rename = "refreshToken", alias = "refresh_token")]
    refresh_token: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChangePasswordInput {
    #[serde(rename = "oldPassword", alias = "old_password")]
    old_password: String,
    #[serde(rename = "newPassword", alias = "new_password")]
    new_password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EmailInput {
    email: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ResetPasswordInput {
    token: String,
    password: String,
    #[serde(alias = "passwordConfirmation")]
    password_confirmation: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OtpInput {
    #[serde(alias = "token")]
    otp: String,
}

/// `POST /auth/register` → 201 `data.user`. Creates the `user` role on first use and
/// emails a verification link (the token is stored on the row).
async fn register(State(state): State<AppState>, Json(input): Json<RegisterInput>) -> ApiResult {
    let username = input.username.trim();
    let email = input.email.trim().to_lowercase();
    let first_name = input.first_name.trim();
    let last_name = input.last_name.trim();
    if [username, email.as_str(), first_name, last_name]
        .iter()
        .any(|value| value.is_empty())
        || input.password.is_empty()
    {
        return Err(ApiError::bad_request(
            "first_name, last_name, username, email and password are required",
        ));
    }
    if !email.contains('@') {
        return Err(ApiError::bad_request("A valid email is required"));
    }
    check_password(&input.password, input.password_confirmation.as_deref())?;
    if User::username_or_email_taken(&state.db, username, &email).await? {
        return Err(ApiError::conflict("Username or email already exists"));
    }

    let role = Role::first_or_create(&state.db, Role::USER).await?;
    let password_hash = hash_password(&input.password, state.config.bcrypt_cost).await?;
    let verify_token = random_token();
    let user = User::create(
        &state.db,
        NewUser {
            username,
            first_name,
            last_name,
            email: &email,
            phone: input
                .phone
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty()),
            password_hash: &password_hash,
            role_id: role.id,
            verify_token: &verify_token,
        },
    )
    .await?;
    send_verification_mail(&state, &user, &verify_token);

    Ok(created(
        "User registered successfully; a verification email has been sent",
        json!({ "user": user }),
    ))
}

/// `POST /auth/login {usernameOrEmail, password}` → `data.token`, `data.refreshToken`, `data.user`
async fn login(State(state): State<AppState>, Json(input): Json<LoginInput>) -> ApiResult {
    let login = input.username_or_email.trim();
    if login.is_empty() || input.password.is_empty() {
        return Err(ApiError::bad_request(
            "usernameOrEmail and password are required",
        ));
    }
    let invalid = || ApiError::unauthorized("Invalid username/email or password");
    let user = User::find_by_login(&state.db, login)
        .await?
        .ok_or_else(invalid)?;
    if !verify_password(&input.password, &user.password).await? {
        return Err(invalid());
    }
    let (token, refresh_token) = issue_token_pair(&state.config, user.id)?;
    Ok(ok(
        "Logged in successfully",
        json!({
            "token": token,
            "refreshToken": refresh_token,
            "is_otp_enabled": user.is_otp_enabled,
            "user": user,
        }),
    ))
}

/// `POST /auth/logout` — tokens are stateless; the client drops them.
async fn logout(_auth: AuthUser) -> ApiResult {
    Ok(message("Logged out successfully"))
}

/// `POST /auth/check-token {token}` (Bearer) — 200 while the access token is valid.
async fn check_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TokenInput>,
) -> ApiResult {
    let token = bearer_token(&headers).unwrap_or(input.token.trim());
    let user_id = verify_token(&state.config, token, TokenKind::Access)?;
    if !User::is_active(&state.db, user_id).await? {
        return Err(ApiError::unauthorized("Invalid or expired token"));
    }
    Ok(ok("Token is valid", json!({ "valid": true })))
}

/// `POST /auth/refresh-token {refreshToken}` → a fresh `data.token` + `data.refreshToken`
async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RefreshInput>,
) -> ApiResult {
    let raw = match input.refresh_token.trim() {
        "" => bearer_token(&headers).unwrap_or_default(),
        token => token,
    };
    let user_id = verify_token(&state.config, raw, TokenKind::Refresh)?;
    if !User::is_active(&state.db, user_id).await? {
        return Err(ApiError::unauthorized("Invalid or expired token"));
    }
    let (token, refresh_token) = issue_token_pair(&state.config, user_id)?;
    Ok(ok(
        "Token refreshed successfully",
        json!({ "token": token, "refreshToken": refresh_token }),
    ))
}

/// `POST /auth/change-password {oldPassword, newPassword}` (Bearer)
async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<ChangePasswordInput>,
) -> ApiResult {
    if input.old_password.is_empty() {
        return Err(ApiError::bad_request("oldPassword is required"));
    }
    check_password(&input.new_password, None)?;
    let user = current(&state, auth).await?;
    if !verify_password(&input.old_password, &user.password).await? {
        return Err(ApiError::bad_request("Current password is incorrect"));
    }
    let hash = hash_password(&input.new_password, state.config.bcrypt_cost).await?;
    User::update_password(&state.db, user.id, &hash).await?;
    Ok(message("Password changed successfully"))
}

/// `POST /auth/forgot-password {email}` — stores a reset token on the user and emails
/// it. Always the same answer, so accounts can't be enumerated.
async fn forgot_password(
    State(state): State<AppState>,
    Json(input): Json<EmailInput>,
) -> ApiResult {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::bad_request("Email is required"));
    }
    if let Some(user) = User::find_by_email(&state.db, &email).await? {
        let token = random_token();
        User::set_reset_token(&state.db, user.id, &token).await?;
        let link = format!("{}/reset-password?token={token}", state.config.frontend_url);
        state.mailer.send(
            &user.email,
            "Reset your CLIuno password",
            format!(
                "Hi {},\n\nReset your password here: {link}\n\nReset token: {token}\n\n\
                 If you didn't ask for this, ignore this email.\n",
                user.first_name
            ),
        );
    }
    Ok(message(
        "If that email is registered, a password reset link has been sent",
    ))
}

/// `POST /auth/reset-password {password, token}` — the user is found by the token.
async fn reset_password(
    State(state): State<AppState>,
    Json(input): Json<ResetPasswordInput>,
) -> ApiResult {
    let token = input.token.trim();
    if token.is_empty() {
        return Err(ApiError::bad_request("Reset token is required"));
    }
    check_password(&input.password, input.password_confirmation.as_deref())?;
    let user = User::find_by_reset_token(&state.db, token)
        .await?
        .ok_or_else(|| ApiError::bad_request("Invalid or expired reset token"))?;
    let hash = hash_password(&input.password, state.config.bcrypt_cost).await?;
    User::update_password(&state.db, user.id, &hash).await?;
    Ok(message("Password reset successfully"))
}

/// `POST /auth/send-verify-email` (Bearer) — stores a new verify token and emails it.
async fn send_verify_email(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let user = current(&state, auth).await?;
    if user.is_verified {
        return Ok(message("Email is already verified"));
    }
    let token = random_token();
    User::set_verify_token(&state.db, user.id, &token).await?;
    send_verification_mail(&state, &user, &token);
    Ok(message("Verification email sent"))
}

/// `POST /auth/verify-email {token}` — the user is found by the token.
async fn verify_email(State(state): State<AppState>, Json(input): Json<TokenInput>) -> ApiResult {
    let token = input.token.trim();
    if token.is_empty() {
        return Err(ApiError::bad_request("Verification token is required"));
    }
    let user = User::find_by_verify_token(&state.db, token)
        .await?
        .ok_or_else(|| ApiError::bad_request("Invalid or expired verification token"))?;
    User::mark_verified(&state.db, user.id).await?;
    Ok(message("Email verified successfully"))
}

/// `POST /auth/otp/generate` (Bearer) → `data.secret` (base32) + `data.otpauth_url`.
/// OTP stays disabled until `otp/verify` accepts a code.
async fn otp_generate(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let user = current(&state, auth).await?;
    let totp = totp_for(Secret::generate(), &user)?;
    let secret = totp.secret().to_base32();
    let url = totp
        .to_url()
        .map_err(|err| ApiError::internal(format!("cannot build otpauth url: {err}")))?;
    User::set_otp_secret(&state.db, user.id, &secret, &url).await?;
    Ok(ok(
        "OTP secret generated",
        json!({ "secret": secret, "base32": secret, "otpauth_url": url }),
    ))
}

/// `POST /auth/otp/verify {otp}` (Bearer) — enables OTP once a code matches.
async fn otp_verify(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<OtpInput>,
) -> ApiResult {
    let user = current(&state, auth).await?;
    let base32 = user
        .otp_base32
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("OTP has not been generated for this user"))?;
    check_otp(base32, &user, &input.otp)?;
    let user = User::enable_otp(&state.db, user.id).await?;
    Ok(ok(
        "OTP verified and enabled",
        json!({ "otp_verified": true, "user": user }),
    ))
}

/// `POST /auth/otp/validate {otp}` (Bearer) — checks a code against enabled OTP.
async fn otp_validate(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<OtpInput>,
) -> ApiResult {
    let user = current(&state, auth).await?;
    let base32 = user
        .otp_base32
        .as_deref()
        .filter(|_| user.is_otp_enabled)
        .ok_or_else(|| ApiError::bad_request("OTP is not enabled for this user"))?;
    check_otp(base32, &user, &input.otp)?;
    Ok(ok("OTP is valid", json!({ "otp_valid": true })))
}

/// `POST /auth/otp/disable` (Bearer) — clears the secret.
async fn otp_disable(State(state): State<AppState>, auth: AuthUser) -> ApiResult {
    let user = User::disable_otp(&state.db, auth.id).await?;
    Ok(ok(
        "OTP disabled",
        json!({ "otp_disabled": true, "user": user }),
    ))
}

async fn current(state: &AppState, auth: AuthUser) -> Result<User, ApiError> {
    User::find(&state.db, auth.id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))
}

fn check_password(password: &str, confirmation: Option<&str>) -> Result<(), ApiError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::bad_request(format!(
            "Password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    if confirmation.is_some_and(|confirmation| confirmation != password) {
        return Err(ApiError::bad_request(
            "Password confirmation does not match",
        ));
    }
    Ok(())
}

fn send_verification_mail(state: &AppState, user: &User, token: &str) {
    let link = format!("{}/verify-email?token={token}", state.config.frontend_url);
    state.mailer.send(
        &user.email,
        "Verify your CLIuno email",
        format!(
            "Hi {},\n\nVerify your email here: {link}\n\nVerification token: {token}\n",
            user.first_name
        ),
    );
}

/// TOTP per RFC 6238 with the defaults every CLIuno backend uses: SHA1, 6 digits,
/// 30-second steps, ±1 step of clock skew.
fn totp_for(secret: Secret, user: &User) -> Result<Totp, ApiError> {
    Builder::new()
        .with_secret(secret)
        .with_issuer(Some(OTP_ISSUER))
        .with_account_name(user.username.replace(':', ""))
        .build()
        .map_err(|err| ApiError::internal(format!("invalid TOTP setup: {err}")))
}

fn check_otp(base32: &str, user: &User, code: &str) -> Result<(), ApiError> {
    let secret = Secret::try_from_base32(base32)
        .map_err(|err| ApiError::internal(format!("stored OTP secret is invalid: {err}")))?;
    let code = code.trim();
    if code.is_empty() {
        return Err(ApiError::bad_request("otp is required"));
    }
    totp_for(secret, user)?
        .check_current(code)
        .map(|_| ())
        .ok_or_else(|| ApiError::bad_request("Invalid OTP code"))
}
