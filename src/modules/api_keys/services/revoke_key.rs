//! Revoking a key.

use super::ApiKeysService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::api_keys::error::ApiKeysError;
use uuid::Uuid;

impl ApiKeysService {
    pub async fn revoke(&self, actor: &SessionUser, id: Uuid) -> AppResult<()> {
        if !self.repo.revoke(actor.0.id, id).await? {
            return Err(ApiKeysError::NotFound.into());
        }
        Ok(())
    }
}
