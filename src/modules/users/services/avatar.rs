//! The profile photo reference, for the avatars feature.

use super::UsersService;
use crate::{common::error::AppResult, modules::users::error::UsersError};
use uuid::Uuid;

impl UsersService {
    /// Returns the previous avatar file name (if any) so the caller can delete the file.
    pub async fn set_avatar_key(
        &self,
        id: Uuid,
        avatar_key: Option<&str>,
    ) -> AppResult<Option<String>> {
        Ok(self
            .repo
            .set_avatar_key(id, avatar_key)
            .await?
            .ok_or(UsersError::NotFound)?)
    }
}
