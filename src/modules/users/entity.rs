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
}

impl User {
    pub fn is_email_verified(&self) -> bool {
        self.email_verified_at.is_some()
    }
}
