//! Removing a linked provider account.

use super::OAuthService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::oauth::{error::OAuthError, provider::Provider};

impl OAuthService {
    /// Removes the link, unless it is the account's only way to sign in.
    pub async fn unlink(&self, actor: &SessionUser, provider: Provider) -> AppResult<()> {
        let linked = self
            .repo
            .list_identities(actor.0.id)
            .await?
            .into_iter()
            .any(|i| i.provider == provider);
        if !linked {
            return Err(OAuthError::NotLinked.into());
        }
        if self.users.sign_in_method_count(actor.0.id).await? < 2 {
            return Err(OAuthError::LastSignInMethod.into());
        }
        self.repo.delete_identity(actor.0.id, provider).await?;
        Ok(())
    }
}
