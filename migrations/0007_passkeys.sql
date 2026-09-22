CREATE TABLE passkeys (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    credential_id   TEXT NOT NULL UNIQUE,
    credential_json TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    last_used_at    TEXT
);

CREATE INDEX idx_passkeys_user ON passkeys (user_id);

-- Server-side state between "begin" and "finish" of a WebAuthn ceremony (single use, short lived).
CREATE TABLE webauthn_challenges (
    id         TEXT PRIMARY KEY NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    user_id    TEXT REFERENCES users(id) ON DELETE CASCADE,
    state_json TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
