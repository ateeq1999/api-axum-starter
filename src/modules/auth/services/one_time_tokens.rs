use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    common::error::AppResult,
    modules::auth::{
        entity::{AuthToken, TokenPurpose},
        error::AuthError,
        helpers::one_time_token,
        repositories::AuthTokenRepository,
    },
};

/// Issues and redeems the single-use links behind reset / verify / email-change flows.
#[derive(Clone)]
pub struct OneTimeTokenService {
    repo: AuthTokenRepository,
}

impl OneTimeTokenService {
    pub fn new(repo: AuthTokenRepository) -> Self {
        Self { repo }
    }

    /// Returns the raw token (only ever put it in an email). Older unused tokens of the same
    /// kind for this user stop working.
    pub async fn issue(
        &self,
        user_id: Uuid,
        purpose: TokenPurpose,
        ttl: Duration,
        new_email: Option<&str>,
    ) -> AppResult<String> {
        self.repo.invalidate_unused(user_id, purpose).await?;
        let token = one_time_token::generate();
        self.repo
            .insert(user_id, purpose, &token.hash, new_email, Utc::now() + ttl)
            .await?;
        Ok(token.raw)
    }

    /// Consumes the token: valid exactly once, and only before it expires.
    pub async fn redeem(&self, purpose: TokenPurpose, raw: &str) -> AppResult<AuthToken> {
        let hash = one_time_token::hash(raw);
        Ok(self
            .repo
            .claim(purpose, &hash)
            .await?
            .ok_or(AuthError::InvalidOrExpiredLink)?)
    }

    /// Checks the token is valid without consuming it, for a flow that needs to retry a second
    /// check (e.g. a TOTP code) before the token should actually be spent — see
    /// `services::totp::verify_login`, which peeks, checks the code, then only calls
    /// [`Self::redeem`] once that succeeds.
    pub async fn peek(&self, purpose: TokenPurpose, raw: &str) -> AppResult<AuthToken> {
        let hash = one_time_token::hash(raw);
        Ok(self
            .repo
            .find_unused(purpose, &hash)
            .await?
            .ok_or(AuthError::InvalidOrExpiredLink)?)
    }

    pub async fn invalidate(&self, user_id: Uuid, purpose: TokenPurpose) -> AppResult<()> {
        self.repo.invalidate_unused(user_id, purpose).await
    }

    pub async fn issued_within(
        &self,
        user_id: Uuid,
        purpose: TokenPurpose,
        window: Duration,
    ) -> AppResult<i64> {
        self.repo
            .count_issued_since(user_id, purpose, Utc::now() - window)
            .await
    }
}
