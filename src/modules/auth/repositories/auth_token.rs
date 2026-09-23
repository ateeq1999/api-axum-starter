use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::error::AppResult,
    modules::auth::entity::{AuthToken, TokenPurpose},
};

#[derive(Clone)]
pub struct AuthTokenRepository {
    db: PgPool,
}

impl AuthTokenRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn insert(
        &self,
        user_id: Uuid,
        purpose: TokenPurpose,
        token_hash: &str,
        new_email: Option<&str>,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO auth_tokens (id, user_id, purpose, token_hash, new_email, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(purpose)
        .bind(token_hash)
        .bind(new_email)
        .bind(expires_at)
        .bind(Utc::now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Marks every still-unused token of this kind as used, so only the newest link works.
    pub async fn invalidate_unused(&self, user_id: Uuid, purpose: TokenPurpose) -> AppResult<()> {
        sqlx::query(
            "UPDATE auth_tokens SET used_at = $1 WHERE user_id = $2 AND purpose = $3 AND used_at IS NULL",
        )
        .bind(Utc::now())
        .bind(user_id)
        .bind(purpose)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Looks up a valid (unused, unexpired) token without consuming it — for flows where the
    /// caller may need to retry a second check (e.g. a TOTP code) before the token should
    /// actually be spent. Prefer [`Self::claim`] whenever a peek-then-spend split is not needed.
    pub async fn find_unused(
        &self,
        purpose: TokenPurpose,
        token_hash: &str,
    ) -> AppResult<Option<AuthToken>> {
        Ok(sqlx::query_as::<_, AuthToken>(
            "SELECT id, user_id, purpose, token_hash, new_email, expires_at, used_at, created_at
             FROM auth_tokens
             WHERE token_hash = $1 AND purpose = $2 AND used_at IS NULL AND expires_at > $3",
        )
        .bind(token_hash)
        .bind(purpose)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }

    /// Atomically finds a valid (unused, unexpired) token and marks it used.
    /// Two concurrent redemptions of the same link cannot both succeed.
    pub async fn claim(
        &self,
        purpose: TokenPurpose,
        token_hash: &str,
    ) -> AppResult<Option<AuthToken>> {
        let now = Utc::now();
        Ok(sqlx::query_as::<_, AuthToken>(
            "UPDATE auth_tokens SET used_at = $1
             WHERE token_hash = $2 AND purpose = $3 AND used_at IS NULL AND expires_at > $1
             RETURNING id, user_id, purpose, token_hash, new_email, expires_at, used_at, created_at",
        )
            .bind(now)
            .bind(token_hash)
            .bind(purpose)
            .fetch_optional(&self.db)
            .await?)
    }

    pub async fn count_issued_since(
        &self,
        user_id: Uuid,
        purpose: TokenPurpose,
        since: DateTime<Utc>,
    ) -> AppResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM auth_tokens WHERE user_id = $1 AND purpose = $2 AND created_at > $3",
        )
        .bind(user_id)
        .bind(purpose)
        .bind(since)
        .fetch_one(&self.db)
        .await?)
    }
}
