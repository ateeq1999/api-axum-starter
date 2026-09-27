use super::one_time_tokens::OneTimeTokenService;
use crate::{
    common::{error::AppResult, security::JwtSettings},
    modules::{
        auth::repositories::TotpRecoveryCodeRepository,
        users::{User, UsersError, UsersService},
    },
};

mod begin_setup;
mod confirm_setup;
mod disable;
mod verify_login;

/// TOTP-based two-factor authentication: enrollment (`begin_setup`/`confirm_setup`), turning it
/// back off (`disable`), and completing a login that a correct password alone was not enough for
/// (`verify_login`). The password check itself and issuing the pending token stay in
/// `SessionService::login`; this only owns what happens with/to the second factor.
#[derive(Clone)]
pub struct TotpService {
    users: UsersService,
    tokens: OneTimeTokenService,
    recovery_codes: TotpRecoveryCodeRepository,
    jwt: JwtSettings,
    /// Shown as the "issuer" in the authenticator app (next to the account email).
    issuer: String,
}

impl TotpService {
    pub fn new(
        users: UsersService,
        tokens: OneTimeTokenService,
        recovery_codes: TotpRecoveryCodeRepository,
        jwt: JwtSettings,
        issuer: String,
    ) -> Self {
        Self {
            users,
            tokens,
            recovery_codes,
            jwt,
            issuer,
        }
    }

    async fn require_active_user(&self, id: uuid::Uuid) -> AppResult<User> {
        Ok(self
            .users
            .find_by_id(id)
            .await?
            .filter(|u| u.is_active)
            .ok_or(UsersError::NotFound)?)
    }
}
