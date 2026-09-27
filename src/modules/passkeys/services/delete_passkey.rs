//! Removing a passkey, unless it is the last way to sign in.

use super::PasskeysService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::passkeys::error::PasskeyError;
use uuid::Uuid;

impl PasskeysService {
    /// Removes a passkey, unless it is the account's only way to sign in.
    pub async fn delete(&self, actor: &SessionUser, id: Uuid) -> AppResult<()> {
        let owns_it = self
            .repo
            .list_by_user(actor.0.id)
            .await?
            .iter()
            .any(|p| p.id == id);
        if !owns_it {
            return Err(PasskeyError::NotFound.into());
        }
        if self.users.sign_in_method_count(actor.0.id).await? < 2 {
            return Err(PasskeyError::LastSignInMethod.into());
        }
        self.repo.delete(actor.0.id, id).await?;
        Ok(())
    }
}
