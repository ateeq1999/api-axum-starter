-- avatar_key: file name of the user's profile photo in the upload directory (NULL = none).
ALTER TABLE users ADD COLUMN avatar_key TEXT;
-- password_set: false for accounts created through OAuth that never chose a password.
ALTER TABLE users ADD COLUMN password_set BOOLEAN NOT NULL DEFAULT TRUE;
