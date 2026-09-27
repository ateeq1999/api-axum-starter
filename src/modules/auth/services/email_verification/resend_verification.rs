//! Sending a fresh verification link to the signed-in user.

use super::EmailVerificationService;
use crate::common::{error::AppResult, security::AuthUser};

impl EmailVerificationService {
    /// No-op when the address is already verified.
    pub async fn resend(&self, actor: &AuthUser) -> AppResult<()> {
        let user = self.current_user(actor).await?;
        if user.is_email_verified() {
            return Ok(());
        }
        self.send_verification(&user).await
    }
}
