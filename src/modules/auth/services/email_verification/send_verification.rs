//! Issuing a verification link.

use super::EmailVerificationService;
use crate::{
    common::error::AppResult,
    modules::{auth::entity::TokenPurpose, users::User},
};

impl EmailVerificationService {
    pub async fn send_verification(&self, user: &User) -> AppResult<()> {
        let raw = self
            .tokens
            .issue(
                user.id,
                TokenPurpose::EmailVerification,
                self.verification_ttl,
                None,
            )
            .await?;
        self.mail
            .send_verification(&user.email, &raw, self.verification_ttl);
        Ok(())
    }
}
