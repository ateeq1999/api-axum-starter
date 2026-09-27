use std::sync::Arc;

use tokio::sync::Semaphore;

use super::storage::AvatarStorage;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::{UsersError, UsersService, dto::UserResponse},
};

mod remove_avatar;
mod serve_avatar;
mod set_avatar;

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

    async fn current(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        Ok(self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?
            .into())
    }
}
