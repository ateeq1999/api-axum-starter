use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::entity::ApiKey;
use crate::common::{error::AppResult, security::ApiKeyScope};

macro_rules! columns {
    () => {
        "id, user_id, name, key_prefix, key_hash, scope, expires_at, last_used_at, revoked_at, created_at"
    };
}

pub struct NewApiKey<'a> {
    pub user_id: Uuid,
    pub name: &'a str,
    pub key_prefix: &'a str,
    pub key_hash: &'a str,
    pub scope: ApiKeyScope,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct ApiKeysRepository {
    db: PgPool,
}

impl ApiKeysRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn insert(&self, new: NewApiKey<'_>) -> AppResult<ApiKey> {
        Ok(sqlx::query_as::<_, ApiKey>(concat!(
            "INSERT INTO api_keys (id, user_id, name, key_prefix, key_hash, scope, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING ",
            columns!()
        ))
        .bind(Uuid::new_v4())
        .bind(new.user_id)
        .bind(new.name)
        .bind(new.key_prefix)
        .bind(new.key_hash)
        .bind(new.scope)
        .bind(new.expires_at)
        .bind(Utc::now())
        .fetch_one(&self.db)
        .await?)
    }

    /// The user's keys that have not been revoked, newest first (expired ones included, so the
    /// owner can see and clean them up).
    pub async fn list_active(&self, user_id: Uuid) -> AppResult<Vec<ApiKey>> {
        Ok(sqlx::query_as::<_, ApiKey>(concat!(
            "SELECT ",
            columns!(),
            " FROM api_keys WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC"
        ))
        .bind(user_id)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn find_by_hash(&self, key_hash: &str) -> AppResult<Option<ApiKey>> {
        Ok(sqlx::query_as::<_, ApiKey>(concat!(
            "SELECT ",
            columns!(),
            " FROM api_keys WHERE key_hash = $1"
        ))
        .bind(key_hash)
        .fetch_optional(&self.db)
        .await?)
    }

    /// Revokes one of the user's keys. Returns false if it does not exist or is already revoked.
    pub async fn revoke(&self, user_id: Uuid, id: Uuid) -> AppResult<bool> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = $1 WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
        )
        .bind(Utc::now())
        .bind(id)
        .bind(user_id)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Records use, at most once a minute per key, so authenticating does not write on every request.
    pub async fn touch(&self, id: Uuid, now: DateTime<Utc>) -> AppResult<()> {
        sqlx::query(
            "UPDATE api_keys SET last_used_at = $1
             WHERE id = $2 AND (last_used_at IS NULL OR last_used_at < $3)",
        )
        .bind(now)
        .bind(id)
        .bind(now - Duration::minutes(1))
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
