CREATE TABLE passkeys (
    id              UUID PRIMARY KEY NOT NULL,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    credential_id   TEXT NOT NULL UNIQUE,
    credential_json TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    last_used_at    TIMESTAMPTZ
);

CREATE INDEX idx_passkeys_user ON passkeys (user_id);

-- Server-side state between "begin" and "finish" of a WebAuthn ceremony (single use, short lived).
-- `id` is a hex string generated in Rust (Uuid::new_v4().simple()), kept as TEXT to match.
CREATE TABLE webauthn_challenges (
    id         TEXT PRIMARY KEY NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    state_json TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
