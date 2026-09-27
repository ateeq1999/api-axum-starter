//! One-time codes handed to the frontend after a provider sign-in.

use super::OAuthRepository;
use crate::common::error::AppResult;
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl OAuthRepository {
    pub async fn insert_grant(
        &self,
        code_hash: &str,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("DELETE FROM login_grants WHERE expires_at < $1")
            .bind(now)
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO login_grants (code_hash, user_id, expires_at, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(code_hash)
        .bind(user_id)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Atomically consumes a grant and returns who it was issued for.
    pub async fn claim_grant(&self, code_hash: &str) -> AppResult<Option<Uuid>> {
        Ok(sqlx::query_scalar(
            "UPDATE login_grants SET used_at = $1
             WHERE code_hash = $2 AND used_at IS NULL AND expires_at > $1
             RETURNING user_id",
        )
        .bind(Utc::now())
        .bind(code_hash)
        .fetch_optional(&self.db)
        .await?)
    }
}
