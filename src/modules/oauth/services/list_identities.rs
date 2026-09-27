//! The provider accounts linked to the caller.

use super::OAuthService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::dto::IdentityResponse;

impl OAuthService {
    pub async fn identities(&self, actor: &SessionUser) -> AppResult<Vec<IdentityResponse>> {
        Ok(self
            .repo
            .list_identities(actor.0.id)
            .await?
            .into_iter()
            .map(IdentityResponse::from)
            .collect())
    }
}
