//! Removing a profile photo: the caller's own, or a deleted account's.

use super::AvatarService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::dto::UserResponse,
};

impl AvatarService {
    pub async fn remove(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        if let Some(old) = self.users.set_avatar_key(actor.id, None).await? {
            self.storage.remove(&old).await;
        }
        self.current(actor).await
    }

    /// Deletes an avatar file directly by name, bypassing the DB (the row is already gone or
    /// being anonymized). Used when a user account is deleted.
    pub async fn remove_file(&self, key: &str) {
        self.storage.remove(key).await;
    }
}
