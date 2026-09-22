CREATE TABLE auth_tokens (
    id         UUID PRIMARY KEY NOT NULL,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose    TEXT NOT NULL CHECK (purpose IN ('password_reset', 'email_verification', 'email_change')),
    token_hash TEXT NOT NULL UNIQUE,
    new_email  TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_auth_tokens_user_purpose ON auth_tokens (user_id, purpose);
