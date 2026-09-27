//! Deleting one file.

use super::MediaService;
use crate::common::{error::AppResult, security::AuthUser};
use uuid::Uuid;

impl MediaService {
    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<()> {
        let media = self.find_accessible(actor, id).await?;
        if let Some(storage_key) = self.repo.delete(media.id).await? {
            self.storage.delete(&storage_key).await;
        }
        Ok(())
    }
}
