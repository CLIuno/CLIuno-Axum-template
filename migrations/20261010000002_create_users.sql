-- Users. One-time tokens (reset/verify) and the OTP secret live on the row so
-- reset-password / verify-email can look the user up by token.
CREATE TABLE IF NOT EXISTS users (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    username       TEXT    NOT NULL UNIQUE,
    first_name     TEXT    NOT NULL,
    last_name      TEXT    NOT NULL,
    email          TEXT    NOT NULL UNIQUE,
    phone          TEXT,
    password       TEXT    NOT NULL,
    avatar_url     TEXT,
    role_id        INTEGER REFERENCES roles (id) ON DELETE SET NULL,
    is_verified    INTEGER NOT NULL DEFAULT 0,
    is_deleted     INTEGER NOT NULL DEFAULT 0,
    is_otp_enabled INTEGER NOT NULL DEFAULT 0,
    reset_token    TEXT,
    verify_token   TEXT,
    otp_base32     TEXT,
    otp_auth_url   TEXT,
    created_at     TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_users_reset_token ON users (reset_token);
CREATE INDEX IF NOT EXISTS idx_users_verify_token ON users (verify_token);
