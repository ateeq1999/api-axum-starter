//! Issuing a new key; the secret is shown once.

use super::{ApiKeysService, DISPLAY_PREFIX_LEN, MAX_ACTIVE_KEYS_PER_USER};
use crate::common::{
    error::AppResult,
    security::{SessionUser, api_key::API_KEY_PREFIX, secret_token},
};
use crate::modules::api_keys::{
    dto::{CreateApiKeyDto, CreatedApiKeyResponse},
    error::ApiKeysError,
    repository::NewApiKey,
};
use chrono::{Duration, Utc};

impl ApiKeysService {
    pub async fn create(
        &self,
        actor: &SessionUser,
        dto: CreateApiKeyDto,
    ) -> AppResult<CreatedApiKeyResponse> {
        let user_id = actor.0.id;
        if self.repo.list_active(user_id).await?.len() as i64 >= MAX_ACTIVE_KEYS_PER_USER {
            return Err(ApiKeysError::LimitReached(MAX_ACTIVE_KEYS_PER_USER).into());
        }

        let generated = secret_token::generate();
        let key = format!("{API_KEY_PREFIX}{}", generated.raw);
        let key_prefix = format!("{API_KEY_PREFIX}{}", &generated.raw[..DISPLAY_PREFIX_LEN]);
        let stored = self
            .repo
            .insert(NewApiKey {
                user_id,
                name: dto.name.trim(),
                key_prefix: &key_prefix,
                // The hash covers the whole key (prefix included), which is what clients send.
                key_hash: &secret_token::hash(&key),
                scope: dto.scope.unwrap_or_default(),
                expires_at: dto
                    .expires_in_days
                    .map(|days| Utc::now() + Duration::days(i64::from(days))),
            })
            .await?;

        Ok(CreatedApiKeyResponse {
            api_key: stored.into(),
            key,
        })
    }
}
