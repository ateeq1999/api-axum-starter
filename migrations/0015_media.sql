-- User-uploaded files. The bytes live in object storage (local disk or S3) under `storage_key`;
-- this row is the ownership and metadata record. `content_type` is what the server detected from
-- the file's own bytes, never what the client claimed.
CREATE TABLE media (
    id                UUID PRIMARY KEY,
    owner_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    storage_key       TEXT NOT NULL UNIQUE,
    content_type      TEXT NOT NULL,
    size_bytes        BIGINT NOT NULL CHECK (size_bytes >= 0),
    original_filename TEXT,
    created_at        TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_media_owner_created ON media (owner_id, created_at DESC);
