ALTER TABLE users ADD COLUMN display_name      TEXT;
ALTER TABLE users ADD COLUMN role              TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin'));
ALTER TABLE users ADD COLUMN is_active         INTEGER NOT NULL DEFAULT 1;
ALTER TABLE users ADD COLUMN email_verified_at TEXT;
ALTER TABLE users ADD COLUMN updated_at        TEXT;
ALTER TABLE users ADD COLUMN deleted_at        TEXT;
