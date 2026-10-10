# syntax=docker/dockerfile:1

# ---- build: compile a release binary (SQLite is bundled, no system libs needed)
FROM rust:1.96-slim-trixie AS builder
WORKDIR /app

# Cache dependencies separately from the app sources.
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY migrations ./migrations
COPY src ./src
# Make sure cargo rebuilds the crate itself, not the cached placeholder.
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

# ---- runtime: just the binary (migrations are embedded in it)
FROM debian:trixie-slim AS runtime
RUN useradd --system --uid 10001 --home-dir /app app \
    && mkdir -p /app/data /app/uploads \
    && chown -R app /app
WORKDIR /app
COPY --from=builder /app/target/release/cliuno-axum-template /usr/local/bin/cliuno-axum-template

USER app
ENV PORT=3000 \
    DATABASE_URL=sqlite:///app/data/cliuno.db \
    UPLOAD_DIR=/app/uploads
EXPOSE 3000
VOLUME ["/app/data", "/app/uploads"]

CMD ["cliuno-axum-template"]
