//! Stored passkey credentials.

use super::PasskeysRepository;
use crate::common::error::AppResult;
use crate::modules::passkeys::entity::PasskeyRow;
use chrono::Utc;
use uuid::Uuid;

impl PasskeysRepository {
    pub async fn list_by_user(&self, user_id: Uuid) -> AppResult<Vec<PasskeyRow>> {
        Ok(sqlx::query_as::<_, PasskeyRow>(concat!(
            "SELECT ",
            columns!(),
            " FROM passkeys WHERE user_id = $1 ORDER BY created_at"
        ))
        .bind(user_id)
        .fetch_all(&self.db)
        .await?)
    }

    /// Returns false if this credential is already registered (to anyone).
    pub async fn insert(
        &self,
        user_id: Uuid,
        name: &str,
        credential_id: &str,
        credential_json: &str,
    ) -> AppResult<Option<PasskeyRow>> {
        Ok(sqlx::query_as::<_, PasskeyRow>(concat!(
            "INSERT INTO passkeys (id, user_id, name, credential_id, credential_json, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (credential_id) DO NOTHING
             RETURNING ",
            columns!()
        ))
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(name)
        .bind(credential_id)
        .bind(credential_json)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }

    /// Persists the updated signature counter after a successful sign-in.
    pub async fn update_after_use(&self, id: Uuid, credential_json: &str) -> AppResult<()> {
        sqlx::query("UPDATE passkeys SET credential_json = $1, last_used_at = $2 WHERE id = $3")
            .bind(credential_json)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, user_id: Uuid, id: Uuid) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM passkeys WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}
