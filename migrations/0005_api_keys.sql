CREATE TABLE api_keys (
    id           TEXT PRIMARY KEY NOT NULL,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    key_prefix   TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    scope        TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    expires_at   TEXT,
    last_used_at TEXT,
    revoked_at   TEXT,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_api_keys_user ON api_keys (user_id);
