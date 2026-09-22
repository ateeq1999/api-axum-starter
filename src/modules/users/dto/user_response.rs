use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{common::security::Role, modules::users::entity::User};

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub role: Role,
    pub is_active: bool,
    pub email_verified: bool,
    /// Relative URL of the profile photo (public, cacheable), or null.
    pub avatar_url: Option<String>,
    /// `false` for accounts created through OAuth that never chose a password.
    pub has_password: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            email_verified: u.is_email_verified(),
            avatar_url: u.avatar_key.as_deref().map(avatar_url),
            has_password: u.password_set,
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            role: u.role,
            is_active: u.is_active,
            created_at: u.created_at,
        }
    }
}

/// Public URL (relative to the API host) of a stored avatar file.
pub fn avatar_url(key: &str) -> String {
    format!("/api/v1/avatars/{key}")
}
