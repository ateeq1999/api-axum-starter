//! Linked provider accounts.

use super::OAuthRepository;
use crate::common::error::AppResult;
use crate::modules::oauth::{entity::OAuthIdentity, provider::Provider};
use chrono::Utc;
use uuid::Uuid;

impl OAuthRepository {
    pub async fn find_identity(
        &self,
        provider: Provider,
        provider_user_id: &str,
    ) -> AppResult<Option<OAuthIdentity>> {
        Ok(sqlx::query_as::<_, OAuthIdentity>(concat!(
            "SELECT ",
            identity_columns!(),
            " FROM oauth_identities WHERE provider = $1 AND provider_user_id = $2"
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
            " FROM oauth_identities WHERE user_id = $1 ORDER BY created_at"
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
            "INSERT INTO oauth_identities
                 (id, user_id, provider, provider_user_id, email, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (provider, provider_user_id) DO NOTHING",
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
        let result =
            sqlx::query("DELETE FROM oauth_identities WHERE user_id = $1 AND provider = $2")
                .bind(user_id)
                .bind(provider)
                .execute(&self.db)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}
