-- TOTP-based two-factor authentication. `totp_secret` is set (but `totp_enabled` stays false)
-- while setup is in progress, so an abandoned setup attempt never blocks a normal password login.
ALTER TABLE users ADD COLUMN totp_secret  TEXT;
ALTER TABLE users ADD COLUMN totp_enabled BOOLEAN NOT NULL DEFAULT FALSE;

-- Single-use codes issued when 2FA is enabled, for signing in if the authenticator device is
-- lost. Only hashes are stored; the raw codes are shown to the user exactly once.
CREATE TABLE totp_recovery_codes (
    id         UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash  TEXT NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_totp_recovery_codes_user_id ON totp_recovery_codes (user_id);

-- A password login for a 2FA-enabled account issues a short-lived "prove the second factor"
-- token (see modules::auth::services::totp) instead of a real access token straight away.
ALTER TABLE auth_tokens DROP CONSTRAINT auth_tokens_purpose_check;
ALTER TABLE auth_tokens ADD CONSTRAINT auth_tokens_purpose_check
    CHECK (purpose IN ('password_reset', 'email_verification', 'email_change', 'two_factor_pending'));
