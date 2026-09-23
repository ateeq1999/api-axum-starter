use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::error::AppResult;

#[derive(Clone)]
pub struct TotpRecoveryCodeRepository {
    db: PgPool,
}

impl TotpRecoveryCodeRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Replaces whatever recovery codes existed for this user with a fresh batch (enabling 2FA
    /// again after a previous disable must not leave old codes still valid).
    pub async fn replace_all(&self, user_id: Uuid, code_hashes: &[String]) -> AppResult<()> {
        let mut tx = self.db.begin().await?;
        sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let now = Utc::now();
        for hash in code_hashes {
            sqlx::query(
                "INSERT INTO totp_recovery_codes (id, user_id, code_hash, created_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(hash)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Atomically consumes a matching, not-yet-used code. `true` if one matched.
    pub async fn claim(&self, user_id: Uuid, code_hash: &str) -> AppResult<bool> {
        let result = sqlx::query(
            "UPDATE totp_recovery_codes SET used_at = $1
             WHERE user_id = $2 AND code_hash = $3 AND used_at IS NULL",
        )
        .bind(Utc::now())
        .bind(user_id)
        .bind(code_hash)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_all(&self, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
