//! The caller's passkeys.

use super::PasskeysService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::passkeys::dto::PasskeyResponse;

impl PasskeysService {
    pub async fn list(&self, actor: &SessionUser) -> AppResult<Vec<PasskeyResponse>> {
        Ok(self
            .repo
            .list_by_user(actor.0.id)
            .await?
            .into_iter()
            .map(PasskeyResponse::from)
            .collect())
    }
}
