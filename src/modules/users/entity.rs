use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::common::security::Role;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub role: Role,
    pub is_active: bool,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub avatar_key: Option<String>,
    pub password_set: bool,
    /// Bumped on every password change; embedded in JWTs at issue time so an old token stops
    /// working immediately once the password changes, instead of staying valid until it expires.
    pub token_version: i32,
    pub failed_login_attempts: i32,
    /// Set once `failed_login_attempts` crosses the configured threshold; login is rejected
    /// (with the same generic "invalid credentials" message, even for the correct password)
    /// until this passes.
    pub locked_until: Option<DateTime<Utc>>,
    /// Set while TOTP two-factor setup is in progress or completed. Only actually required at
    /// login when `totp_enabled` is true — an abandoned setup attempt never blocks sign-in.
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
}

impl User {
    pub fn is_email_verified(&self) -> bool {
        self.email_verified_at.is_some()
    }
}
