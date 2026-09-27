//! Two-factor (TOTP) state.

use uuid::Uuid;

use super::UsersRepository;
use crate::common::error::AppResult;

impl UsersRepository {
    /// Stores a freshly generated secret while two-factor setup is in progress. Does not turn
    /// two-factor on by itself — see [`Self::enable_totp`].
    pub async fn set_pending_totp_secret(&self, id: Uuid, base32_secret: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET totp_secret = $1 WHERE id = $2")
            .bind(base32_secret)
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Turns two-factor on for the secret already stored by [`Self::set_pending_totp_secret`].
    pub async fn enable_totp(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET totp_enabled = TRUE WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Turns two-factor off and forgets the secret entirely (a new enrollment starts fresh).
    pub async fn disable_totp(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET totp_enabled = FALSE, totp_secret = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
