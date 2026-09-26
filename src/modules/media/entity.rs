use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Media {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub storage_key: String,
    /// Detected from the file's bytes at upload time.
    pub content_type: String,
    pub size_bytes: i64,
    pub original_filename: Option<String>,
    pub created_at: DateTime<Utc>,
}
