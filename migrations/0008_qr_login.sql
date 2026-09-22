CREATE TABLE qr_sessions (
    id              TEXT PRIMARY KEY NOT NULL,
    secret_hash     TEXT NOT NULL,
    code            TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'scanned', 'approved', 'rejected', 'consumed')),
    user_id         TEXT REFERENCES users(id) ON DELETE CASCADE,
    requester_ip    TEXT,
    requester_agent TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    expires_at      TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
