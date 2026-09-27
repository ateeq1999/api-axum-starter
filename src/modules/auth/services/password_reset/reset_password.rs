//! Setting a new password from a reset or invitation link.

use super::PasswordResetService;
use crate::{
    common::error::AppResult,
    modules::auth::{dto::ResetPasswordDto, entity::TokenPurpose, error::AuthError},
};

impl PasswordResetService {
    pub async fn reset(&self, dto: ResetPasswordDto) -> AppResult<()> {
        let token = self
            .tokens
            .redeem(TokenPurpose::PasswordReset, &dto.token)
            .await?;
        let user = self
            .users
            .find_by_id(token.user_id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(AuthError::InvalidOrExpiredLink)?;

        self.users.set_password(user.id, dto.new_password).await?;
        // Following the emailed link proves control of the inbox.
        self.users.mark_email_verified(user.id).await?;
        self.tokens
            .invalidate(user.id, TokenPurpose::PasswordReset)
            .await?;
        self.mail.send_password_changed(&user.email);
        metrics::counter!("password_reset_completed_total").increment(1);
        Ok(())
    }
}
