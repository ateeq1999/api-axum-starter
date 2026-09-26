use axum::body::Bytes;
use uuid::Uuid;

use super::{
    dto::MediaResponse,
    entity::Media,
    error::MediaError,
    repository::{MediaRepository, NewMedia},
};
use crate::{
    common::{
        dto::{PaginatedResponse, Pagination},
        error::AppResult,
        security::AuthUser,
    },
    config::MediaConfig,
    infra::storage::ObjectStorage,
};

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

    /// Stores an upload for the caller. The type is detected from the bytes themselves and
    /// checked against the allow-list; whatever `Content-Type` the client sent is ignored.
    pub async fn upload(
        &self,
        actor: &AuthUser,
        upload: Bytes,
        original_filename: Option<String>,
    ) -> AppResult<MediaResponse> {
        if upload.is_empty() {
            return Err(MediaError::Empty.into());
        }
        if upload.len() > self.config.max_upload_bytes {
            return Err(MediaError::TooLarge(self.config.max_upload_bytes / (1024 * 1024)).into());
        }
        let detected = infer::get(&upload).ok_or(MediaError::UnrecognizedType)?;
        let content_type = detected.mime_type();
        if !self
            .config
            .allowed_content_types
            .iter()
            .any(|allowed| allowed == content_type)
        {
            return Err(MediaError::TypeNotAllowed(content_type.to_string()).into());
        }

        let storage_key = format!(
            "{KEY_PREFIX}{}.{}",
            Uuid::new_v4().simple(),
            detected.extension()
        );
        let size_bytes = upload.len() as i64;
        self.storage
            .put(&storage_key, upload.to_vec(), content_type)
            .await?;

        let inserted = self
            .repo
            .insert(NewMedia {
                owner_id: actor.id,
                storage_key: &storage_key,
                content_type,
                size_bytes,
                original_filename: original_filename.as_deref(),
            })
            .await;
        match inserted {
            Ok(media) => Ok(media.into()),
            Err(error) => {
                self.storage.delete(&storage_key).await;
                Err(error)
            }
        }
    }

    pub async fn list(
        &self,
        actor: &AuthUser,
        pagination: Pagination,
    ) -> AppResult<PaginatedResponse<MediaResponse>> {
        let (items, total) = self
            .repo
            .list_by_owner(actor.id, pagination.limit(), pagination.offset())
            .await?;
        Ok(PaginatedResponse::new(
            items.into_iter().map(MediaResponse::from).collect(),
            pagination,
            total,
        ))
    }

    pub async fn metadata(&self, actor: &AuthUser, id: Uuid) -> AppResult<MediaResponse> {
        Ok(self.find_accessible(actor, id).await?.into())
    }

    pub async fn content(&self, actor: &AuthUser, id: Uuid) -> AppResult<MediaContent> {
        let media = self.find_accessible(actor, id).await?;
        let Some(bytes) = self.storage.get(&media.storage_key).await? else {
            tracing::warn!(media_id = %media.id, "media row has no stored object");
            return Err(MediaError::NotFound.into());
        };
        Ok(MediaContent { media, bytes })
    }

    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<()> {
        let media = self.find_accessible(actor, id).await?;
        if let Some(storage_key) = self.repo.delete(media.id).await? {
            self.storage.delete(&storage_key).await;
        }
        Ok(())
    }

    /// Removes everything a user uploaded. Used when their account is deleted.
    pub async fn delete_all_for_user(&self, user_id: Uuid) -> AppResult<()> {
        for storage_key in self.repo.delete_all_by_owner(user_id).await? {
            self.storage.delete(&storage_key).await;
        }
        Ok(())
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
