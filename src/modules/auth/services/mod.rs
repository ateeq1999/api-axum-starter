pub mod email_verification;
pub mod one_time_tokens;
pub mod password_reset;
pub mod session;
pub mod totp;

use sqlx::PgPool;

use self::{
    email_verification::EmailVerificationService, one_time_tokens::OneTimeTokenService,
    password_reset::PasswordResetService, session::SessionService, totp::TotpService,
};
use super::repositories::{AuthTokenRepository, TotpRecoveryCodeRepository};
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
    pub totp: TotpService,
}

impl AuthService {
    pub fn new(db: PgPool, users: UsersService, mail: MailService, config: &Config) -> Self {
        let tokens = OneTimeTokenService::new(AuthTokenRepository::new(db.clone()));
        let email_verification = EmailVerificationService::new(
            users.clone(),
            mail.clone(),
            tokens.clone(),
            config.account.clone(),
        );
        let totp = TotpService::new(
            users.clone(),
            tokens.clone(),
            TotpRecoveryCodeRepository::new(db),
            config.jwt.clone(),
            config.webauthn.rp_name.clone(),
        );
        Self {
            session: SessionService::new(
                users.clone(),
                email_verification.clone(),
                tokens.clone(),
                config.jwt.clone(),
                config.account.clone(),
            ),
            password_reset: PasswordResetService::new(users, mail, tokens, config.account.clone()),
            email_verification,
            totp,
        }
    }
}
