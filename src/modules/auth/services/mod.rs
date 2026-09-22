pub mod email_verification;
pub mod one_time_tokens;
pub mod password_reset;
pub mod session;

use sqlx::PgPool;

use self::{
    email_verification::EmailVerificationService, one_time_tokens::OneTimeTokenService,
    password_reset::PasswordResetService, session::SessionService,
};
use super::repositories::AuthTokenRepository;
use crate::{
    config::Config,
    modules::{mail::MailService, users::UsersService},
};

/// Facade so controllers inject one type.
#[derive(Clone)]
pub struct AuthService {
    pub session: SessionService,
    pub password_reset: PasswordResetService,
    pub email_verification: EmailVerificationService,
}

impl AuthService {
    pub fn new(db: PgPool, users: UsersService, mail: MailService, config: &Config) -> Self {
        let tokens = OneTimeTokenService::new(AuthTokenRepository::new(db));
        let email_verification = EmailVerificationService::new(
            users.clone(),
            mail.clone(),
            tokens.clone(),
            config.account.clone(),
        );
        Self {
            session: SessionService::new(
                users.clone(),
                email_verification.clone(),
                config.jwt.clone(),
                config.account.require_verified_email,
            ),
            password_reset: PasswordResetService::new(users, mail, tokens, config.account.clone()),
            email_verification,
        }
    }
}
