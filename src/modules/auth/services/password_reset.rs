use chrono::Duration;

use super::one_time_tokens::OneTimeTokenService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, Role, password},
    },
    config::AccountConfig,
    modules::{
        auth::{
            dto::{ChangePasswordDto, ForgotPasswordDto, InviteUserDto, ResetPasswordDto},
            entity::TokenPurpose,
            error::AuthError,
            helpers::one_time_token,
        },
        mail::MailService,
        users::{UsersError, UsersService, dto::UserResponse},
    },
};

/// At most this many reset emails per account per hour; extra requests are silently ignored.
const MAX_RESETS_PER_HOUR: i64 = 3;

#[derive(Clone)]
pub struct PasswordResetService {
    users: UsersService,
    mail: MailService,
    tokens: OneTimeTokenService,
    reset_ttl: Duration,
    invitation_ttl: Duration,
}

impl PasswordResetService {
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
            reset_ttl: Duration::minutes(config.password_reset_ttl_minutes),
            invitation_ttl: Duration::hours(config.email_verification_ttl_hours),
        }
    }

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
        Ok(())
    }

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
        Ok(())
    }

    pub async fn change(&self, actor: &AuthUser, dto: ChangePasswordDto) -> AppResult<()> {
        let user = self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?;

        let password_ok =
            password::verify_blocking(dto.current_password, user.password_hash.clone()).await?;
        if !password_ok {
            return Err(AuthError::WrongCurrentPassword.into());
        }

        self.users.set_password(user.id, dto.new_password).await?;
        self.mail.send_password_changed(&user.email);
        Ok(())
    }

    /// Admin invitation: creates the account with an unguessable password and emails a link to
    /// choose a real one.
    pub async fn invite(&self, dto: InviteUserDto) -> AppResult<UserResponse> {
        let unusable_password = one_time_token::generate().raw;
        let user = self
            .users
            .create_with_password(
                &dto.email,
                unusable_password,
                dto.display_name.as_deref(),
                dto.role.unwrap_or(Role::User),
                false,
            )
            .await?;

        let raw = self
            .tokens
            .issue(
                user.id,
                TokenPurpose::PasswordReset,
                self.invitation_ttl,
                None,
            )
            .await?;
        self.mail
            .send_invitation(&user.email, &raw, self.invitation_ttl);
        Ok(user.into())
    }
}
