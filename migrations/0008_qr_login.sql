-- `id` is a random string generated in Rust (not a UUID), kept as TEXT to match.
CREATE TABLE qr_sessions (
    id              TEXT PRIMARY KEY NOT NULL,
    secret_hash     TEXT NOT NULL,
    code            TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'scanned', 'approved', 'rejected', 'consumed')),
    user_id         UUID REFERENCES users(id) ON DELETE CASCADE,
    requester_ip    TEXT,
    requester_agent TEXT,
    attempts        BIGINT NOT NULL DEFAULT 0,
    expires_at      TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL
);
