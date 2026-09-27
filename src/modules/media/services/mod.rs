use uuid::Uuid;

use super::{entity::Media, error::MediaError, repository::MediaRepository};
use crate::{
    common::{error::AppResult, security::AuthUser},
    config::MediaConfig,
    infra::storage::ObjectStorage,
};

mod delete_media;
mod delete_user_media;
mod download_content;
mod get_metadata;
mod list_media;
mod upload_media;

const KEY_PREFIX: &str = "media/";

/// A stored file's metadata together with its bytes.
pub struct MediaContent {
    pub media: Media,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct MediaService {
    repo: MediaRepository,
    storage: ObjectStorage,
    config: MediaConfig,
}

impl MediaService {
    pub fn new(repo: MediaRepository, storage: ObjectStorage, config: MediaConfig) -> Self {
        Self {
            repo,
            storage,
            config,
        }
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.config.max_upload_bytes
    }

    /// The owner, or an admin. Anyone else gets `NotFound`, so a file's existence is not leaked.
    async fn find_accessible(&self, actor: &AuthUser, id: Uuid) -> AppResult<Media> {
        self.repo
            .find_by_id(id)
            .await?
            .filter(|media| media.owner_id == actor.id || actor.role.is_admin())
            .ok_or_else(|| MediaError::NotFound.into())
    }
}
