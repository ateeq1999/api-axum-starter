//! The caller's active keys (never the secret).

use super::ApiKeysService;
use crate::common::{error::AppResult, security::SessionUser};
use crate::modules::api_keys::dto::ApiKeyResponse;

impl ApiKeysService {
    pub async fn list(&self, actor: &SessionUser) -> AppResult<Vec<ApiKeyResponse>> {
        Ok(self
            .repo
            .list_active(actor.0.id)
            .await?
            .into_iter()
            .map(ApiKeyResponse::from)
            .collect())
    }
}
