//! Trading the one-time code from the callback for an access token.

use super::OAuthService;
use crate::modules::oauth::error::OAuthError;
use crate::{
    common::{
        error::AppResult,
        security::{jwt, secret_token},
    },
    modules::auth::dto::TokenResponse,
};

impl OAuthService {
    /// `POST /auth/oauth/exchange`: trades the one-time code for an access token.
    pub async fn exchange(&self, code: &str) -> AppResult<TokenResponse> {
        let user_id = self
            .repo
            .claim_grant(&secret_token::hash(code))
            .await?
            .ok_or(OAuthError::InvalidGrant)?;
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(OAuthError::AccountDisabled)?;

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }
}
