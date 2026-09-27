//! Password hash and the failed-login counter.

use chrono::Utc;
use uuid::Uuid;

use super::UsersRepository;
use crate::common::error::AppResult;

impl UsersRepository {
    /// Also bumps `token_version`, so every token issued before this call stops working
    /// immediately (see the column's doc comment on [`super::entity::User`]).
    pub async fn set_password_hash(&self, id: Uuid, password_hash: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET password_hash = $1, password_set = TRUE, token_version = token_version + 1, updated_at = $2
             WHERE id = $3 AND deleted_at IS NULL",
        )
            .bind(password_hash)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Counts one wrong password attempt and, once `max_attempts` is reached, locks the account
    /// until `now + lockout`. Done in one statement so a burst of concurrent wrong-password
    /// requests cannot each read a stale (pre-increment) count and all decide not to lock.
    pub async fn record_failed_login(
        &self,
        id: Uuid,
        max_attempts: i32,
        lockout: chrono::Duration,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET
                 failed_login_attempts = failed_login_attempts + 1,
                 locked_until = CASE
                     WHEN failed_login_attempts + 1 >= $2 THEN $3
                     ELSE locked_until
                 END
             WHERE id = $1",
        )
        .bind(id)
        .bind(max_attempts)
        .bind(Utc::now() + lockout)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Called once the correct password is confirmed, so a legitimate sign-in (even one that is
    /// then rejected for another reason, e.g. an unverified email) always clears the counter.
    pub async fn reset_failed_logins(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE users SET failed_login_attempts = 0, locked_until = NULL WHERE id = $1",
        )
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
