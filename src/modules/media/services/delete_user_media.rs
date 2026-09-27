//! Deleting everything a user uploaded, when their account is deleted.

use super::MediaService;
use crate::common::error::AppResult;
use uuid::Uuid;

impl MediaService {
    /// Removes everything a user uploaded. Used when their account is deleted.
    pub async fn delete_all_for_user(&self, user_id: Uuid) -> AppResult<()> {
        for storage_key in self.repo.delete_all_by_owner(user_id).await? {
            self.storage.delete(&storage_key).await;
        }
        Ok(())
    }
}
