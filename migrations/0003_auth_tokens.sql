CREATE TABLE auth_tokens (
    id         TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose    TEXT NOT NULL CHECK (purpose IN ('password_reset', 'email_verification', 'email_change')),
    token_hash TEXT NOT NULL UNIQUE,
    new_email  TEXT,
    expires_at TEXT NOT NULL,
    used_at    TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_auth_tokens_user_purpose ON auth_tokens (user_id, purpose);
