-- One row per in-flight authorization request; deleted when the provider redirects back.
CREATE TABLE oauth_states (
    state_hash    TEXT PRIMARY KEY NOT NULL,
    provider      TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    user_id       TEXT REFERENCES users(id) ON DELETE CASCADE,   -- set when linking an existing account
    redirect_path TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE oauth_identities (
    id               TEXT PRIMARY KEY NOT NULL,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider         TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    email            TEXT,
    created_at       TEXT NOT NULL,
    UNIQUE (provider, provider_user_id)
);

CREATE INDEX idx_oauth_identities_user ON oauth_identities (user_id);

-- Short-lived single-use codes that the SPA exchanges for an access token after an OAuth redirect.
CREATE TABLE login_grants (
    code_hash  TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    used_at    TEXT,
    created_at TEXT NOT NULL
);
