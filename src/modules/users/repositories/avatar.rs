//! The profile photo reference.

use chrono::Utc;
use uuid::Uuid;

use super::UsersRepository;
use crate::common::error::AppResult;

impl UsersRepository {
    /// Sets (or clears) the profile photo. Returns the previous file name so the caller can
    /// delete the old file.
    pub async fn set_avatar_key(
        &self,
        id: Uuid,
        avatar_key: Option<&str>,
    ) -> AppResult<Option<Option<String>>> {
        let mut tx = self.db.begin().await?;
        let previous: Option<Option<String>> =
            sqlx::query_scalar("SELECT avatar_key FROM users WHERE id = $1 AND deleted_at IS NULL")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if previous.is_none() {
            return Ok(None);
        }
        sqlx::query("UPDATE users SET avatar_key = $1, updated_at = $2 WHERE id = $3")
            .bind(avatar_key)
            .bind(Utc::now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(previous)
    }
}
