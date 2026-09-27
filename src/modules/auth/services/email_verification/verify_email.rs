//! Confirming an email address from its link.

use super::EmailVerificationService;
use crate::{
    common::error::AppResult,
    modules::auth::{dto::VerifyEmailDto, entity::TokenPurpose},
};

impl EmailVerificationService {
    pub async fn verify(&self, dto: VerifyEmailDto) -> AppResult<()> {
        let token = self
            .tokens
            .redeem(TokenPurpose::EmailVerification, &dto.token)
            .await?;
        self.users.mark_email_verified(token.user_id).await
    }
}
