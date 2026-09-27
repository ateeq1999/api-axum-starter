//! Storing an upload after checking its real type and size.

use super::{KEY_PREFIX, MediaService};
use crate::common::{error::AppResult, security::AuthUser};
use crate::modules::media::{dto::MediaResponse, error::MediaError, repository::NewMedia};
use axum::body::Bytes;
use uuid::Uuid;

impl MediaService {
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
}
