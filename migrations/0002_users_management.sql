ALTER TABLE users ADD COLUMN display_name      TEXT;
ALTER TABLE users ADD COLUMN role              TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin'));
ALTER TABLE users ADD COLUMN is_active         BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN updated_at        TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN deleted_at        TIMESTAMPTZ;
