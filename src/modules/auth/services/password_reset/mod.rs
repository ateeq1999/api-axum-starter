use chrono::Duration;

use super::one_time_tokens::OneTimeTokenService;
use crate::{
    config::AccountConfig,
    modules::{mail::MailService, users::UsersService},
};

mod change_password;
mod forgot_password;
mod invite_user;
mod reset_password;

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
}
