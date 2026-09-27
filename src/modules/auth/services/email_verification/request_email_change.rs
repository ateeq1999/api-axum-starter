//! Asking to change the email: link to the new address, notice to the old.

use super::EmailVerificationService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, password},
    },
    modules::{
        auth::{dto::ChangeEmailDto, entity::TokenPurpose, error::AuthError},
        users::{UsersError, normalize_email},
    },
};

impl EmailVerificationService {
    /// Starts an email change: a confirmation link goes to the NEW address and a notice to the
    /// OLD one. Nothing changes until the link is used.
    pub async fn request_change(&self, actor: &AuthUser, dto: ChangeEmailDto) -> AppResult<()> {
        let user = self.current_user(actor).await?;

        let password_ok =
            password::verify_blocking(dto.current_password, user.password_hash.clone()).await?;
        if !password_ok {
            return Err(AuthError::WrongCurrentPassword.into());
        }

        let new_email = normalize_email(&dto.new_email);
        if new_email == user.email {
            return Err(AuthError::EmailUnchanged.into());
        }
        // Authenticated endpoint, so telling the caller the address is taken is not enumeration.
        if self.users.find_by_email(&new_email).await?.is_some() {
            return Err(UsersError::EmailTaken.into());
        }

        let raw = self
            .tokens
            .issue(
                user.id,
                TokenPurpose::EmailChange,
                self.change_ttl,
                Some(&new_email),
            )
            .await?;
        self.mail
            .send_email_change_confirm(&new_email, &raw, self.change_ttl);
        self.mail.send_email_change_notice(&user.email, &new_email);
        Ok(())
    }
}
