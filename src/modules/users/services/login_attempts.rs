//! Counting failed sign-ins and the lockout, for the auth feature.

use super::UsersService;
use crate::common::error::AppResult;
use uuid::Uuid;

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
}
