use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::entity::Media;
use crate::common::error::AppResult;

macro_rules! columns {
    () => {
        "id, owner_id, storage_key, content_type, size_bytes, original_filename, created_at"
    };
}

pub struct NewMedia<'a> {
    pub owner_id: Uuid,
    pub storage_key: &'a str,
    pub content_type: &'a str,
    pub size_bytes: i64,
    pub original_filename: Option<&'a str>,
}

#[derive(Clone)]
pub struct MediaRepository {
    db: PgPool,
}

impl MediaRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn insert(&self, new: NewMedia<'_>) -> AppResult<Media> {
        Ok(sqlx::query_as::<_, Media>(concat!(
            "INSERT INTO media (id, owner_id, storage_key, content_type, size_bytes, original_filename, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING ",
            columns!()
        ))
        .bind(Uuid::new_v4())
        .bind(new.owner_id)
        .bind(new.storage_key)
        .bind(new.content_type)
        .bind(new.size_bytes)
        .bind(new.original_filename)
        .bind(Utc::now())
        .fetch_one(&self.db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<Media>> {
        Ok(
            sqlx::query_as::<_, Media>(concat!("SELECT ", columns!(), " FROM media WHERE id = $1"))
                .bind(id)
                .fetch_optional(&self.db)
                .await?,
        )
    }

    /// The owner's files, newest first, with the total count for pagination.
    pub async fn list_by_owner(
        &self,
        owner_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Media>, u64)> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media WHERE owner_id = $1")
            .bind(owner_id)
            .fetch_one(&self.db)
            .await?;
        let items = sqlx::query_as::<_, Media>(concat!(
            "SELECT ",
            columns!(),
            " FROM media WHERE owner_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2 OFFSET $3"
        ))
        .bind(owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;
        Ok((items, total.max(0) as u64))
    }

    /// Returns the storage key of the deleted row, or `None` if it did not exist.
    pub async fn delete(&self, id: Uuid) -> AppResult<Option<String>> {
        Ok(
            sqlx::query_scalar("DELETE FROM media WHERE id = $1 RETURNING storage_key")
                .bind(id)
                .fetch_optional(&self.db)
                .await?,
        )
    }

    /// Deletes every row the user owns and returns their storage keys.
    pub async fn delete_all_by_owner(&self, owner_id: Uuid) -> AppResult<Vec<String>> {
        Ok(
            sqlx::query_scalar("DELETE FROM media WHERE owner_id = $1 RETURNING storage_key")
                .bind(owner_id)
                .fetch_all(&self.db)
                .await?,
        )
    }
}
