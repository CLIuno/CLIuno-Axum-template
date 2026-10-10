# CLIuno Axum template

The CLIuno demo app's REST API on **Rust + [Axum](https://github.com/tokio-rs/axum)** with
SQLite: JWT auth (refresh tokens, password reset, email verification, TOTP two-factor),
users, todos, posts with comments, follows, roles and image uploads — the same contract
every [CLIuno](https://github.com/CLIuno) frontend speaks, under `/api/v1`.

## Stack

Axum 0.8 · Tokio · tower-http (CORS, tracing, static files) · sqlx 0.9 (SQLite, runtime
queries, embedded migrations) · jsonwebtoken · bcrypt · totp-rs · lettre · serde · tracing.

## Quick start

```bash
cp .env.example .env     # optional — every setting has a development default
cargo run                # http://localhost:3000/api/v1
```

The SQLite file (`cliuno.db`) is created and migrated on first boot — no seed step. The
first registration creates the `user` role.

```bash
curl -X POST localhost:3000/api/v1/auth/register -H 'Content-Type: application/json' \
  -d '{"first_name":"Ada","last_name":"Lovelace","username":"ada","email":"ada@example.com","phone":"1","password":"password123","password_confirmation":"password123"}'
curl -X POST localhost:3000/api/v1/auth/login -H 'Content-Type: application/json' \
  -d '{"usernameOrEmail":"ada","password":"password123"}'
```

## Configuration

| Variable                                   | Default                     | Purpose                                         |
| ------------------------------------------ | --------------------------- | ----------------------------------------------- |
| `PORT`                                     | `3000`                      | HTTP port                                       |
| `DATABASE_URL`                             | `sqlite://cliuno.db`        | SQLite database (created if missing)            |
| `JWT_SECRET_KEY` / `REFRESH_JWT_SECRET_KEY` | development secrets         | HS256 secrets for access / refresh tokens       |
| `ACCESS_TOKEN_TTL` / `REFRESH_TOKEN_TTL`   | `3600` / `604800`           | token lifetimes (seconds)                       |
| `BCRYPT_COST`                              | `10`                        | password hashing cost                           |
| `PUBLIC_BASE_URL`                          | request `Host`              | origin used in upload URLs                      |
| `UPLOAD_DIR`                               | `uploads`                   | where images are stored and served from         |
| `FRONTEND_URL`                             | `http://localhost:5173`     | base of links in emails                         |
| `MAIL_HOST` / `MAIL_PORT`                  | unset / `1025`              | plain SMTP (unset host = mails are only logged) |
| `MAIL_USERNAME` / `MAIL_PASSWORD`          | unset                       | SMTP credentials                                |
| `MAIL_FROM_ADDRESS`                        | `no-reply@cliuno.local`     | sender                                          |
| `RUST_LOG`                                 | `cliuno_axum_template=info` | log filter                                      |

## API

Every response is `{ "status": "success" | "error", "message": "...", "data": { ... } }`.
Authenticated routes take `Authorization: Bearer <data.token from login>`.

| Area      | Endpoints                                                                                                                                                                       |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auth      | `POST /auth/{register,login,logout,check-token,refresh-token,change-password,forgot-password,reset-password,send-verify-email,verify-email}`, `POST /auth/otp/{generate,verify,validate,disable}` |
| Users     | `GET /users`, `GET\|PATCH\|DELETE /users/current`, `GET /users/username/:username`, `GET /users/:id` (`PATCH`/`DELETE` admin-only), `GET /users/:id/posts`, `GET /users/:id/roles` |
| Todos     | `GET\|POST /todos`, `GET /todos/current-user`, `GET\|PATCH\|DELETE /todos/:id`, `PATCH /todos/:id/toggle`                                                                        |
| Posts     | `GET\|POST /posts`, `GET /posts/current-user`, `GET\|PATCH\|DELETE /posts/:id`, `GET /posts/:id/user`, `GET\|POST /posts/:id/comments`, `PATCH\|DELETE /posts/:id/comments/:commentId` |
| Follows   | `POST\|DELETE /follows/:id/follow`, `GET /follows/:id/{followers,following,is-following}`                                                                                       |
| Roles     | `GET\|POST /roles`, `GET\|PATCH\|DELETE /roles/:id`, `GET /roles/:id/users` (admin-only)                                                                                         |
| Uploads   | `POST /uploads/image` (multipart `file`, png/jpeg/webp/gif ≤ 5 MB) → `data.url`; files served at `GET /uploads/<file>`                                                           |

To make a user an admin:

```bash
sqlite3 cliuno.db "INSERT OR IGNORE INTO roles (name) VALUES ('admin');
  UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') WHERE email = 'ada@example.com';"
```

## Project layout

```
migrations/            schema, one <timestamp>_<name>.sql per table (embedded, applied on boot)
src/main.rs            boot: config, pool, migrations, serve
src/app.rs             the Router (api, /uploads, CORS, tracing)
src/routes/*.rs        one module per resource: handlers + routes()
src/models/*.rs        one module per table: FromRow structs + queries
src/auth.rs            JWT, bcrypt, AuthUser / AdminUser extractors
src/error.rs           ApiError -> {status, message} with the right HTTP code
src/response.rs        the success envelope
src/extract.rs         Json / Path extractors with envelope rejections
tests/                 integration tests against an in-memory database
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Docker

```bash
docker build -t cliuno-axum .
docker run --rm -p 3000:3000 -v cliuno-data:/app/data -v cliuno-uploads:/app/uploads cliuno-axum
```

## License

[AGPL-3.0](LICENSE)
