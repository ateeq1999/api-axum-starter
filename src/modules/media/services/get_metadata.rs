//! One file's metadata.

use super::MediaService;
use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::dto::MediaResponse;
use uuid::Uuid;

impl MediaService {
    pub async fn metadata(&self, actor: &AuthUser, id: Uuid) -> AppResult<MediaResponse> {
        Ok(self.find_accessible(actor, id).await?.into())
    }
}
