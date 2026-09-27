//! Two-factor (TOTP) state, for the auth feature.

use super::UsersService;
use crate::common::error::AppResult;
use uuid::Uuid;

impl UsersService {
    pub async fn set_pending_totp_secret(&self, id: Uuid, base32_secret: &str) -> AppResult<()> {
        self.repo.set_pending_totp_secret(id, base32_secret).await
    }

    pub async fn enable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.enable_totp(id).await
    }

    pub async fn disable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.disable_totp(id).await
    }
}
