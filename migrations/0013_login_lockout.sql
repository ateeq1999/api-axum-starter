-- Per-account brute-force protection, independent of the per-IP rate limiter (which a
-- distributed credential-stuffing attempt, many IPs against one account, does not slow down).
ALTER TABLE users ADD COLUMN failed_login_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN locked_until           TIMESTAMPTZ;
