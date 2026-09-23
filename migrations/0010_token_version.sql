-- token_version: bumped whenever the password changes, embedded in every JWT issued afterwards.
-- A JWT whose `tv` claim no longer matches the row is rejected, closing the gap where a token
-- issued before a password change kept working (unlike an API key, which is checked live).
ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 1;
