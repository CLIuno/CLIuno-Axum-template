# CLIuno Axum template

Rust + Axum 0.8 REST API serving the CLIuno contract under `/api/v1` on SQLite (sqlx):
JWT auth (refresh, password reset, email verification, OTP), users, todos, posts+comments,
follows, roles, image uploads.

## Commands

```bash
cargo run                                   # dev server on :3000 (PORT), creates cliuno.db + runs migrations
cargo test                                  # integration tests (tests/*.rs, in-memory SQLite) + unit tests
cargo fmt --check                           # rustfmt
cargo clippy --all-targets -- -D warnings   # lint (CI fails on any warning)
cargo build --release                       # target/release/cliuno-axum-template
```

Copy `.env.example` → `.env` (optional: every setting has a development default).
Delete `cliuno.db*` for a clean slate.

## Structure

- `src/main.rs` — boot only: dotenv, tracing, `Config::from_env`, `db::connect` +
  `db::migrate`, `axum::serve` with graceful shutdown.
- `src/lib.rs` — module list; `src/app.rs` — `pub fn router(state) -> Router`
  (`/` health, `/api/v1` = `routes::api()`, public `/uploads` via `ServeDir`, JSON 404
  fallback, permissive CORS, `TraceLayer`). Tests build this exact router.
- `src/config.rs` (env → `Config`, plus `Config::for_tests`), `src/state.rs`
  (`AppState { db, config, mailer }`), `src/db.rs` (pool, `sqlx::migrate!`),
  `src/mail.rs` (lettre, plain SMTP, sends on a background task).
- `src/error.rs` — `ApiError` → `{status: "error", message}` with the right HTTP code;
  `ApiResult<T = ApiResponse>`; `From<sqlx::Error>` (RowNotFound → 404, unique → 409).
- `src/response.rs` — `ok(msg, json!({...}))` (200), `created(...)` (201), `message(msg)`.
- `src/extract.rs` — `Json<T>` (lenient: empty body = `{}`, no content-type check) and
  `Path<T>` (unparseable id → 404) whose rejections use the envelope. Use these, not axum's.
- `src/auth.rs` — HS256 access/refresh tokens (separate secrets, `kind` claim), bcrypt on
  `spawn_blocking`, `random_token()`, the `AuthUser` extractor (Bearer → active user,
  else 401) with `ensure_owns(&row)` (403), and `AdminUser` (role `admin`, else 403).
- `src/models/<table>.rs` — row struct (`FromRow` + `Serialize`), request structs
  (`Deserialize`, `#[serde(default)]`), and `impl` blocks with every query (runtime
  `sqlx::query_as`, never the compile-time `query!` macros). `models/mod.rs` has the
  `Owned` trait and `with_owner(s)` which embed the owner as `user`.
- `src/routes/<resource>.rs` — `pub fn routes() -> Router<AppState>` + handlers;
  registered in `src/routes/mod.rs` (`pub mod x;` + `.merge(x::routes())`).
- `migrations/<YYYYMMDDHHMMSS>_<name>.sql` — schema, embedded at compile time and applied
  on boot (`build.rs` makes cargo rebuild when a migration is added).
- `tests/common/mod.rs` — `TestApp` (router + pool over in-memory SQLite, `request`,
  `token_for`); `tests/{auth,todos,social}.rs`.

## Contract rules this codebase follows

- Responses `{status, message, data}`; frontends destructure exact keys (`data.users`,
  `data.user`, `data.todos`, `data.todo`, `data.posts`, `data.post`, `data.comments`,
  `data.comment`, `data.followers`, `data.following`, `data.isFollowing`, `data.role`,
  login `data.token` + `data.refreshToken` + `data.user`, upload `data.url` + `data.filename`).
- Response fields are snake_case (`is_completed`, `image_url`, `avatar_url`, `created_at`);
  todos/posts/comments embed their owner as `user`, posts embed `comments`.
- Request keys: camelCase where the frontends send them (`usernameOrEmail`, `refreshToken`,
  `oldPassword`/`newPassword`, `otp`); resource fields snake_case with camelCase aliases
  (`#[serde(alias = "imageUrl")]`).
- One-time tokens live on the user row (`reset_token`, `verify_token`) and are looked up
  by token; registration stores (and emails) the verify token. Login is not gated on
  verification.
- The `user` role is created on first registration (`Role::first_or_create`) — a fresh
  clone works with an empty database, no seed step.
- OTP endpoints act on the authenticated user (totp-rs, SHA1/6 digits/30 s, ±1 step).
- `GET /users` is auth-not-admin; `PATCH/DELETE /users/:id` and `/roles` are admin-only.
- Creates attach the authenticated user as owner; update/delete check ownership (403).
- Uploads: multipart `file`, png/jpeg/webp/gif sniffed from the bytes, ≤ 5 MB (413),
  stored as `<uuid>.<ext>` in `UPLOAD_DIR`, served unauthenticated at `/uploads/<file>`;
  `data.url` is absolute (`PUBLIC_BASE_URL` or the request Host).

## Adding a resource (mirror `todos`)

1. `migrations/<timestamp>_create_<table>.sql` — `id INTEGER PRIMARY KEY AUTOINCREMENT`,
   `user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE`, columns,
   `created_at`/`updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP`.
2. `src/models/<name>.rs` (copy `todo.rs`) + `pub mod <name>;` in `src/models/mod.rs`.
3. `src/routes/<plural>.rs` (copy `todos.rs`) + `pub mod <plural>;` and
   `.merge(<plural>::routes())` in `src/routes/mod.rs`.
4. `tests/<plural>.rs` using `mod common;` + `TestApp`.

## Notes

- Matrix: `node core/scripts/test-matrix.mjs --backends axum` builds `--release`, runs the
  binary with `DATABASE_URL=sqlite://matrix-test.db`, and reads reset/verify tokens from
  that file with `python3`.
- Make an admin: `sqlite3 cliuno.db "INSERT OR IGNORE INTO roles (name) VALUES ('admin');
  UPDATE users SET role_id = (SELECT id FROM roles WHERE name = 'admin') WHERE email = 'you@example.com';"`
- SQLite runs in WAL mode; JSON key order is preserved (`serde_json/preserve_order`).
- Logout is stateless (tokens expire); the client discards them.

## Conventions

rustfmt + clippy `-D warnings` (CI); edition 2024; conventional commits, no co-author
trailers; mail goes to `MAIL_HOST`/`MAIL_PORT` (mailpit or any SMTP sink locally).

## AI agents (Claude Code and friends)

These commands need **cliuno 2.2 or newer** (`npx cliuno@^2.2` fetches it).

- **Check the CLIuno skills first**: `npx cliuno@^2.2 skills status` — if any is
  `missing`/`outdated`, run `npx cliuno@^2.2 skills install` (adds `.claude/skills/cliuno-*`:
  the generator workflow, the scaffold guide, and the shared REST contract).
- **Generate resources, don't hand-write them**:
  `npx cliuno@^2.2 make:resource Product --fields name:string,price:number,inStock:boolean`
  creates `src/models/product.rs`, `src/routes/products.rs`, `migrations/<timestamp>_create_products.sql`, `tests/products.rs` and registers it in `src/routes/mod.rs`, `src/models/mod.rs`, following this
  template's conventions and the `{status, message, data}` contract. `--dry-run` previews,
  re-running never duplicates registrations, `--force` overwrites.
- Single pieces: `make:model`, `make:controller`, `make:migration`, `make:test`. `npx cliuno@^2.2 make` lists them.
- Then edit fields/business logic only, and run the gates above.
