use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::entity::{ChallengeKind, PasskeyRow};
use crate::common::error::AppResult;

macro_rules! columns {
    () => {
        "id, user_id, name, credential_id, credential_json, created_at, last_used_at"
    };
}

#[derive(Clone)]
pub struct PasskeysRepository {
    db: PgPool,
}

impl PasskeysRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

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

    // ---- ceremony state ----------------------------------------------------------------

    pub async fn insert_challenge(
        &self,
        id: &str,
        kind: ChallengeKind,
        user_id: Option<Uuid>,
        state_json: &str,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("DELETE FROM webauthn_challenges WHERE expires_at < $1")
            .bind(now)
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO webauthn_challenges (id, kind, user_id, state_json, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(kind.as_str())
        .bind(user_id)
        .bind(state_json)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Atomically consumes the ceremony state: a challenge can be answered exactly once.
    /// `user_id` must match (`None` matches only challenges created without a user).
    pub async fn claim_challenge(
        &self,
        id: &str,
        kind: ChallengeKind,
        user_id: Option<Uuid>,
    ) -> AppResult<Option<String>> {
        Ok(sqlx::query_scalar(
            "DELETE FROM webauthn_challenges
             WHERE id = $1 AND kind = $2 AND user_id IS NOT DISTINCT FROM $3 AND expires_at > $4
             RETURNING state_json",
        )
        .bind(id)
        .bind(kind.as_str())
        .bind(user_id)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }
}
