use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::modules::media::entity::Media;

#[derive(Debug, Serialize, ToSchema)]
pub struct MediaResponse {
    pub id: Uuid,
    /// Detected from the file's bytes, not taken from the upload request.
    pub content_type: String,
    pub size_bytes: i64,
    pub original_filename: Option<String>,
    /// Where to fetch the file. It needs the same credentials as any other endpoint.
    pub url: String,
    pub created_at: DateTime<Utc>,
}

impl From<Media> for MediaResponse {
    fn from(media: Media) -> Self {
        Self {
            url: format!("/api/v1/media/{}/content", media.id),
            id: media.id,
            content_type: media.content_type,
            size_bytes: media.size_bytes,
            original_filename: media.original_filename,
            created_at: media.created_at,
        }
    }
}
