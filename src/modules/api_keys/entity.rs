use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::common::security::ApiKeyScope;

#[derive(Debug, Clone, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub scope: ApiKeyScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl ApiKey {
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && self.expires_at.is_none_or(|expires| expires > now)
    }
}
