use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{common::security::ApiKeyScope, modules::api_keys::entity::ApiKey};

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    /// First characters of the key, to recognise it in a list. The full key is never shown again.
    pub key_prefix: String,
    pub scope: ApiKeyScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<ApiKey> for ApiKeyResponse {
    fn from(k: ApiKey) -> Self {
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            scope: k.scope,
            expires_at: k.expires_at,
            last_used_at: k.last_used_at,
            created_at: k.created_at,
        }
    }
}

/// Returned once, when the key is created: the only time the secret is visible.
#[derive(Debug, Serialize)]
pub struct CreatedApiKeyResponse {
    #[serde(flatten)]
    pub api_key: ApiKeyResponse,
    /// Send it as `Authorization: Bearer <key>` or `X-API-Key: <key>`. Store it now.
    pub key: String,
}
