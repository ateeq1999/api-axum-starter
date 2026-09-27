//! Ceremony state between the begin and finish steps.

use super::PasskeysRepository;
use crate::common::error::AppResult;
use crate::modules::passkeys::entity::ChallengeKind;
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl PasskeysRepository {
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
