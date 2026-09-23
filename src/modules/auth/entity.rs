use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TokenPurpose {
    PasswordReset,
    EmailVerification,
    EmailChange,
    /// Issued after a correct password for a two-factor-enabled account, redeemed once the
    /// authenticator code (or a recovery code) is also confirmed. See `services::totp`.
    TwoFactorPending,
}

#[derive(Debug, Clone, FromRow)]
pub struct AuthToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub purpose: TokenPurpose,
    pub token_hash: String,
    pub new_email: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
