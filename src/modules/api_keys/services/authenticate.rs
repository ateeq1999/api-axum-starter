//! Resolving a presented key to its owner, on every API-key request.

use super::ApiKeysService;
use crate::common::{
    error::AppResult,
    security::{
        ApiKeyVerifier, AuthUser, Credential, SecurityError, api_key::BoxFuture, secret_token,
    },
};
use chrono::Utc;

impl ApiKeysService {
    /// Resolves a presented key to its owner. Every failure is the same generic 401 so a caller
    /// cannot tell a wrong key from a revoked, expired or disabled-owner one.
    async fn authenticate(&self, presented: &str) -> AppResult<AuthUser> {
        let now = Utc::now();

        let key = self
            .repo
            .find_by_hash(&secret_token::hash(presented))
            .await?
            .filter(|key| key.is_usable(now))
            .ok_or_else(invalid_key)?;
        // The role is read from the database on every request, so demoting or deactivating the
        // owner takes effect immediately for API keys (unlike JWTs, which live until expiry).
        let owner = self
            .users
            .find_by_id(key.user_id)
            .await?
            .filter(|user| user.is_active)
            .ok_or_else(invalid_key)?;

        self.repo.touch(key.id, now).await?;
        metrics::counter!("api_key_auth_total", "outcome" => "success").increment(1);
        Ok(AuthUser {
            id: owner.id,
            role: owner.role,
            credential: Credential::ApiKey(key.scope),
        })
    }
}

fn invalid_key() -> crate::common::error::AppError {
    metrics::counter!("api_key_auth_total", "outcome" => "failure").increment(1);
    SecurityError::InvalidApiKey.into()
}

impl ApiKeyVerifier for ApiKeysService {
    fn verify<'a>(&'a self, presented: &'a str) -> BoxFuture<'a, AppResult<AuthUser>> {
        Box::pin(self.authenticate(presented))
    }
}
