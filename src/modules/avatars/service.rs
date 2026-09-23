use std::sync::Arc;

use axum::body::Bytes;
use tokio::sync::Semaphore;

use super::{error::AvatarError, processing, storage::AvatarStorage};
use crate::{
    common::{
        error::{AppError, AppResult},
        security::AuthUser,
    },
    modules::users::{UsersError, UsersService, dto::UserResponse},
};

/// Largest upload accepted (the stored result is a ~256x256 JPEG, far smaller).
pub const MAX_UPLOAD_BYTES: usize = 2 * 1024 * 1024;
/// Image decoding is CPU and memory heavy, so only a few run at once.
const MAX_CONCURRENT_PROCESSING: usize = 4;

#[derive(Clone)]
pub struct AvatarService {
    users: UsersService,
    storage: AvatarStorage,
    processing: Arc<Semaphore>,
}

impl AvatarService {
    pub fn new(users: UsersService, storage: AvatarStorage) -> Self {
        Self {
            users,
            storage,
            processing: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSING)),
        }
    }

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

    pub async fn remove(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        if let Some(old) = self.users.set_avatar_key(actor.id, None).await? {
            self.storage.remove(&old).await;
        }
        self.current(actor).await
    }

    pub async fn read(&self, file: &str) -> AppResult<Vec<u8>> {
        self.storage.read(file).await
    }

    /// Deletes an avatar file directly by name, bypassing the DB (the row is already gone or
    /// being anonymized). Used when a user account is deleted.
    pub async fn remove_file(&self, key: &str) {
        self.storage.remove(key).await;
    }

    async fn current(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        Ok(self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?
            .into())
    }
}
