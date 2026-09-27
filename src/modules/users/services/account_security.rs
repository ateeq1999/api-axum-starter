//! Login lockout, two-factor, email and avatar state, for the auth, avatars, oauth and passkeys features.

use uuid::Uuid;

use super::{UsersService, normalize_email};
use crate::{common::error::AppResult, modules::users::error::UsersError};

impl UsersService {
    /// See `UsersRepository::record_failed_login`.
    pub async fn record_failed_login(
        &self,
        id: Uuid,
        max_attempts: i32,
        lockout: chrono::Duration,
    ) -> AppResult<()> {
        self.repo
            .record_failed_login(id, max_attempts, lockout)
            .await
    }

    pub async fn reset_failed_logins(&self, id: Uuid) -> AppResult<()> {
        self.repo.reset_failed_logins(id).await
    }

    pub async fn set_pending_totp_secret(&self, id: Uuid, base32_secret: &str) -> AppResult<()> {
        self.repo.set_pending_totp_secret(id, base32_secret).await
    }

    pub async fn enable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.enable_totp(id).await
    }

    pub async fn disable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.disable_totp(id).await
    }

    pub async fn mark_email_verified(&self, id: Uuid) -> AppResult<()> {
        self.repo.mark_email_verified(id).await
    }

    pub async fn change_email(&self, id: Uuid, new_email: &str) -> AppResult<()> {
        self.repo
            .change_email(id, &normalize_email(new_email))
            .await
    }

    /// Returns the previous avatar file name (if any) so the caller can delete the file.
    pub async fn set_avatar_key(
        &self,
        id: Uuid,
        avatar_key: Option<&str>,
    ) -> AppResult<Option<String>> {
        Ok(self
            .repo
            .set_avatar_key(id, avatar_key)
            .await?
            .ok_or(UsersError::NotFound)?)
    }

    pub async fn sign_in_method_count(&self, id: Uuid) -> AppResult<i64> {
        self.repo.sign_in_method_count(id).await
    }
}
