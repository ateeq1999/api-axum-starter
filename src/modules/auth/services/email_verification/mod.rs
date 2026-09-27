use chrono::Duration;

use super::one_time_tokens::OneTimeTokenService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    config::AccountConfig,
    modules::{
        mail::MailService,
        users::{User, UsersError, UsersService},
    },
};

mod confirm_email_change;
mod request_email_change;
mod resend_verification;
mod send_verification;
mod verify_email;

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

    async fn current_user(&self, actor: &AuthUser) -> AppResult<User> {
        Ok(self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?)
    }
}
