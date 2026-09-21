use chrono::Duration;

use super::one_time_tokens::OneTimeTokenService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, password},
    },
    config::AccountConfig,
    modules::{
        auth::{
            dto::{ChangeEmailDto, VerifyEmailDto},
            entity::TokenPurpose,
            error::AuthError,
        },
        mail::MailService,
        users::{User, UsersError, UsersService, normalize_email},
    },
};

#[derive(Clone)]
pub struct EmailVerificationService {
    users: UsersService,
    mail: MailService,
    tokens: OneTimeTokenService,
    verification_ttl: Duration,
    change_ttl: Duration,
}

impl EmailVerificationService {
    pub fn new(
        users: UsersService,
        mail: MailService,
        tokens: OneTimeTokenService,
        config: AccountConfig,
    ) -> Self {
        Self {
            users,
            mail,
            tokens,
            verification_ttl: Duration::hours(config.email_verification_ttl_hours),
            change_ttl: Duration::minutes(config.email_change_ttl_minutes),
        }
    }

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

    pub async fn verify(&self, dto: VerifyEmailDto) -> AppResult<()> {
        let token = self
            .tokens
            .redeem(TokenPurpose::EmailVerification, &dto.token)
            .await?;
        self.users.mark_email_verified(token.user_id).await
    }

    /// No-op when the address is already verified.
    pub async fn resend(&self, actor: &AuthUser) -> AppResult<()> {
        let user = self.current_user(actor).await?;
        if user.is_email_verified() {
            return Ok(());
        }
        self.send_verification(&user).await
    }

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

    async fn current_user(&self, actor: &AuthUser) -> AppResult<User> {
        Ok(self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?)
    }
}
