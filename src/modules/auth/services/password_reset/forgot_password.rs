//! Emailing a reset link, the same answer whether or not the address exists.

use super::{MAX_RESETS_PER_HOUR, PasswordResetService};
use crate::{
    common::error::AppResult,
    modules::auth::{dto::ForgotPasswordDto, entity::TokenPurpose},
};
use chrono::Duration;

impl PasswordResetService {
    /// Always succeeds from the caller's point of view, whether or not the address is registered.
    pub async fn forgot(&self, dto: ForgotPasswordDto) -> AppResult<()> {
        let Some(user) = self.users.find_by_email(&dto.email).await? else {
            return Ok(());
        };
        if !user.is_active {
            return Ok(());
        }

        let recent = self
            .tokens
            .issued_within(user.id, TokenPurpose::PasswordReset, Duration::hours(1))
            .await?;
        if recent >= MAX_RESETS_PER_HOUR {
            tracing::warn!(user_id = %user.id, "password reset rate limit reached for account");
            return Ok(());
        }

        let raw = self
            .tokens
            .issue(user.id, TokenPurpose::PasswordReset, self.reset_ttl, None)
            .await?;
        self.mail
            .send_password_reset(&user.email, &raw, self.reset_ttl);
        metrics::counter!("password_reset_requested_total").increment(1);
        Ok(())
    }
}
