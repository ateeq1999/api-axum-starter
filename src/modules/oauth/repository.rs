use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{
    entity::{OAuthIdentity, OAuthState},
    provider::Provider,
};
use crate::common::error::AppResult;

macro_rules! identity_columns {
    () => {
        "id, user_id, provider, provider_user_id, email, created_at"
    };
}

#[derive(Clone)]
pub struct OAuthRepository {
    db: SqlitePool,
}

impl OAuthRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    // ---- authorization requests --------------------------------------------------------

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
        sqlx::query("DELETE FROM oauth_states WHERE expires_at < ?")
            .bind(now)
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO oauth_states
                 (state_hash, provider, pkce_verifier, user_id, redirect_path, expires_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
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
            "DELETE FROM oauth_states WHERE state_hash = ? AND expires_at > ?
             RETURNING provider, pkce_verifier, user_id, redirect_path",
        )
        .bind(state_hash)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }

    // ---- linked identities -------------------------------------------------------------

    pub async fn find_identity(
        &self,
        provider: Provider,
        provider_user_id: &str,
    ) -> AppResult<Option<OAuthIdentity>> {
        Ok(sqlx::query_as::<_, OAuthIdentity>(concat!(
            "SELECT ",
            identity_columns!(),
            " FROM oauth_identities WHERE provider = ? AND provider_user_id = ?"
        ))
        .bind(provider)
        .bind(provider_user_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn list_identities(&self, user_id: Uuid) -> AppResult<Vec<OAuthIdentity>> {
        Ok(sqlx::query_as::<_, OAuthIdentity>(concat!(
            "SELECT ",
            identity_columns!(),
            " FROM oauth_identities WHERE user_id = ? ORDER BY created_at"
        ))
        .bind(user_id)
        .fetch_all(&self.db)
        .await?)
    }

    /// Returns false if that provider account is already linked (to anyone).
    pub async fn insert_identity(
        &self,
        user_id: Uuid,
        provider: Provider,
        provider_user_id: &str,
        email: Option<&str>,
    ) -> AppResult<bool> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO oauth_identities
                 (id, user_id, provider, provider_user_id, email, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(provider)
        .bind(provider_user_id)
        .bind(email)
        .bind(Utc::now())
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete_identity(&self, user_id: Uuid, provider: Provider) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM oauth_identities WHERE user_id = ? AND provider = ?")
            .bind(user_id)
            .bind(provider)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ---- login grants (redirect -> access token hand-over) -----------------------------

    pub async fn insert_grant(
        &self,
        code_hash: &str,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("DELETE FROM login_grants WHERE expires_at < ?")
            .bind(now)
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO login_grants (code_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
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
            "UPDATE login_grants SET used_at = ?
             WHERE code_hash = ? AND used_at IS NULL AND expires_at > ?
             RETURNING user_id",
        )
        .bind(Utc::now())
        .bind(code_hash)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }
}
