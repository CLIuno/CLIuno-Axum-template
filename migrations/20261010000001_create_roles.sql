-- Roles. The `user` role is created on first registration (never seeded), so a
-- fresh clone works against an empty database.
CREATE TABLE IF NOT EXISTS roles (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL UNIQUE,
    created_at TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
