use chrono::Duration;

use super::{email_verification::EmailVerificationService, one_time_tokens::OneTimeTokenService};
use crate::{common::security::JwtSettings, config::AccountConfig, modules::users::UsersService};

mod current_user;
mod login;
mod register;

/// How long a caller has, after a correct password for a two-factor-enabled account, to prove
/// the second factor before having to log in again from scratch.
const TWO_FACTOR_PENDING_TTL: Duration = Duration::minutes(5);

#[derive(Clone)]
pub struct SessionService {
    users: UsersService,
    email_verification: EmailVerificationService,
    tokens: OneTimeTokenService,
    jwt: JwtSettings,
    account: AccountConfig,
}

impl SessionService {
    pub fn new(
        users: UsersService,
        email_verification: EmailVerificationService,
        tokens: OneTimeTokenService,
        jwt: JwtSettings,
        account: AccountConfig,
    ) -> Self {
        Self {
            users,
            email_verification,
            tokens,
            jwt,
            account,
        }
    }
}
