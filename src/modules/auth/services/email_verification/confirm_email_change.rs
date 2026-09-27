//! Completing an email change from its link.

use super::EmailVerificationService;
use crate::{
    common::error::AppResult,
    modules::auth::{dto::VerifyEmailDto, entity::TokenPurpose, error::AuthError},
};

impl EmailVerificationService {
    pub async fn confirm_change(&self, dto: VerifyEmailDto) -> AppResult<()> {
        let token = self
            .tokens
            .redeem(TokenPurpose::EmailChange, &dto.token)
            .await?;
        let new_email = token.new_email.ok_or(AuthError::InvalidOrExpiredLink)?;

        // Unique violation (address taken meanwhile) surfaces as UsersError::EmailTaken (409).
        self.users.change_email(token.user_id, &new_email).await?;
        self.tokens
            .invalidate(token.user_id, TokenPurpose::EmailChange)
            .await
    }
}
