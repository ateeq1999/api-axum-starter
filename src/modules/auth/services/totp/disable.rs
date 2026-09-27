//! Turning two-factor off (needs the current password).

use super::TotpService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, password},
    },
    modules::auth::error::AuthError,
};

impl TotpService {
    /// Requires the current password so a stolen session token alone cannot turn two-factor off.
    pub async fn disable(&self, actor: &AuthUser, current_password: String) -> AppResult<()> {
        let user = self.require_active_user(actor.id).await?;
        let password_ok =
            password::verify_blocking(current_password, user.password_hash.clone()).await?;
        if !password_ok {
            return Err(AuthError::WrongCurrentPassword.into());
        }
        self.users.disable_totp(user.id).await?;
        self.recovery_codes.delete_all(user.id).await?;
        Ok(())
    }
}
