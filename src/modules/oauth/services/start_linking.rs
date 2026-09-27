//! Sending a signed-in user to the provider to link it to their account.

use super::OAuthService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::{dto::AuthorizeUrlResponse, provider::Provider};

impl OAuthService {
    /// Start linking a provider to the signed-in user's account.
    pub async fn link_url(
        &self,
        actor: &SessionUser,
        provider: Provider,
    ) -> AppResult<AuthorizeUrlResponse> {
        let authorize_url = self
            .start(provider, Some(actor.0.id), "/settings".to_string())
            .await?;
        Ok(AuthorizeUrlResponse { authorize_url })
    }
}
