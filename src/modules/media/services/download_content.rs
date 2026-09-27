//! One file's bytes.

use super::{MediaContent, MediaService};
use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::error::MediaError;
use uuid::Uuid;

impl MediaService {
    pub async fn content(&self, actor: &AuthUser, id: Uuid) -> AppResult<MediaContent> {
        let media = self.find_accessible(actor, id).await?;
        let Some(bytes) = self.storage.get(&media.storage_key).await? else {
            tracing::warn!(media_id = %media.id, "media row has no stored object");
            return Err(MediaError::NotFound.into());
        };
        Ok(MediaContent { media, bytes })
    }
}
