use chrono::{Duration, Utc};
use uuid::Uuid;

use super::{
    dto::{ApiKeyResponse, CreateApiKeyDto, CreatedApiKeyResponse},
    error::ApiKeysError,
    repository::{ApiKeysRepository, NewApiKey},
};
use crate::{
    common::{
        error::AppResult,
        security::{
            ApiKeyVerifier, AuthUser, Credential, SecurityError, SessionUser,
            api_key::{API_KEY_PREFIX, BoxFuture},
            secret_token,
        },
    },
    modules::users::UsersService,
};

const MAX_ACTIVE_KEYS_PER_USER: i64 = 20;
/// Characters of the secret part shown as the key's recognisable prefix.
const DISPLAY_PREFIX_LEN: usize = 6;

#[derive(Clone)]
pub struct ApiKeysService {
    repo: ApiKeysRepository,
    users: UsersService,
}

impl ApiKeysService {
    pub fn new(repo: ApiKeysRepository, users: UsersService) -> Self {
        Self { repo, users }
    }

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

    pub async fn list(&self, actor: &SessionUser) -> AppResult<Vec<ApiKeyResponse>> {
        Ok(self
            .repo
            .list_active(actor.0.id)
            .await?
            .into_iter()
            .map(ApiKeyResponse::from)
            .collect())
    }

    pub async fn revoke(&self, actor: &SessionUser, id: Uuid) -> AppResult<()> {
        if !self.repo.revoke(actor.0.id, id).await? {
            return Err(ApiKeysError::NotFound.into());
        }
        Ok(())
    }

    /// Resolves a presented key to its owner. Every failure is the same generic 401 so a caller
    /// cannot tell a wrong key from a revoked, expired or disabled-owner one.
    async fn authenticate(&self, presented: &str) -> AppResult<AuthUser> {
        let now = Utc::now();

        let key = self
            .repo
            .find_by_hash(&secret_token::hash(presented))
            .await?
            .filter(|key| key.is_usable(now))
            .ok_or(SecurityError::InvalidApiKey)?;
        // The role is read from the database on every request, so demoting or deactivating the
        // owner takes effect immediately for API keys (unlike JWTs, which live until expiry).
        let owner = self
            .users
            .find_by_id(key.user_id)
            .await?
            .filter(|user| user.is_active)
            .ok_or(SecurityError::InvalidApiKey)?;

        self.repo.touch(key.id, now).await?;
        Ok(AuthUser {
            id: owner.id,
            role: owner.role,
            credential: Credential::ApiKey(key.scope),
        })
    }
}

impl ApiKeyVerifier for ApiKeysService {
    fn verify<'a>(&'a self, presented: &'a str) -> BoxFuture<'a, AppResult<AuthUser>> {
        Box::pin(self.authenticate(presented))
    }
}
