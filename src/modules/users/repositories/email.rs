//! Email verification and email changes.

use chrono::Utc;
use uuid::Uuid;

use super::{UsersRepository, map_write_error};
use crate::common::error::AppResult;

impl UsersRepository {
    pub async fn mark_email_verified(&self, id: Uuid) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $1), updated_at = $2
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Swaps the address and marks it verified (the caller proved control of it).
    pub async fn change_email(&self, id: Uuid, new_email: &str) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE users SET email = $1, email_verified_at = $2, updated_at = $3
             WHERE id = $4 AND deleted_at IS NULL",
        )
        .bind(new_email)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await
        .map_err(map_write_error)?;
        Ok(())
    }
}
