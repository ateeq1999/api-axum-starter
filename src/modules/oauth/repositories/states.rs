//! The short-lived state rows that tie a provider redirect to the request that started it.

use super::OAuthRepository;
use crate::common::error::AppResult;
use crate::modules::oauth::{entity::OAuthState, provider::Provider};
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl OAuthRepository {
    pub async fn insert_state(
        &self,
        state_hash: &str,
        provider: Provider,
        pkce_verifier: &str,
        user_id: Option<Uuid>,
        redirect_path: &str,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now();
        // Opportunistic cleanup of abandoned requests.
        sqlx::query("DELETE FROM oauth_states WHERE expires_at < $1")
            .bind(now)
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO oauth_states
                 (state_hash, provider, pkce_verifier, user_id, redirect_path, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(state_hash)
        .bind(provider)
        .bind(pkce_verifier)
        .bind(user_id)
        .bind(redirect_path)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Atomically consumes the request: a `state` value works exactly once, and only before it expires.
    pub async fn claim_state(&self, state_hash: &str) -> AppResult<Option<OAuthState>> {
        Ok(sqlx::query_as::<_, OAuthState>(
            "DELETE FROM oauth_states WHERE state_hash = $1 AND expires_at > $2
             RETURNING provider, pkce_verifier, user_id, redirect_path",
        )
        .bind(state_hash)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }
}
