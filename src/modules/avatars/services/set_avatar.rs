//! Replacing the caller's profile photo with an uploaded image.

use super::{AvatarService, MAX_UPLOAD_BYTES};
use crate::modules::avatars::{error::AvatarError, processing, storage::AvatarStorage};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::AuthUser,
    },
    modules::users::dto::UserResponse,
};
use axum::body::Bytes;

impl AvatarService {
    /// Replaces the caller's profile photo with the uploaded image.
    pub async fn set(&self, actor: &AuthUser, upload: Bytes) -> AppResult<UserResponse> {
        if upload.len() > MAX_UPLOAD_BYTES {
            return Err(AvatarError::TooLarge(MAX_UPLOAD_BYTES / (1024 * 1024)).into());
        }

        let permit = self
            .processing
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let jpeg = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            processing::to_avatar_jpeg(&upload)
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))??;

        let name = AvatarStorage::new_name();
        self.storage.write(&name, &jpeg).await?;

        match self.users.set_avatar_key(actor.id, Some(&name)).await {
            Ok(previous) => {
                if let Some(old) = previous {
                    self.storage.remove(&old).await;
                }
            }
            Err(error) => {
                self.storage.remove(&name).await;
                return Err(error);
            }
        }
        self.current(actor).await
    }
}
