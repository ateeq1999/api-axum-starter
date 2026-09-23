use chrono::Duration;

use super::{email_verification::EmailVerificationService, one_time_tokens::OneTimeTokenService};
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, JwtSettings, Role, jwt, password},
    },
    config::AccountConfig,
    modules::{
        auth::{
            dto::{CredentialsDto, LoginResponse, TokenResponse},
            entity::TokenPurpose,
            error::AuthError,
        },
        users::{UsersService, dto::UserResponse},
    },
};

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

    pub async fn register(&self, dto: CredentialsDto) -> AppResult<UserResponse> {
        let user = self
            .users
            .create_with_password(&dto.email, dto.password, None, Role::User, false)
            .await?;
        metrics::counter!("auth_register_total").increment(1);

        // The account exists either way; a failed verification email must not fail sign-up.
        if let Err(error) = self.email_verification.send_verification(&user).await {
            tracing::error!(?error, user_id = %user.id, "could not issue verification email");
        }
        Ok(user.into())
    }

    pub async fn login(&self, dto: CredentialsDto) -> AppResult<LoginResponse> {
        let outcome = |result: bool| {
            metrics::counter!("auth_login_total", "outcome" => if result { "success" } else { "failure" }).increment(1)
        };

        let user = match self.users.find_by_email(&dto.email).await? {
            Some(user) => user,
            None => {
                outcome(false);
                return Err(AuthError::InvalidCredentials.into());
            }
        };

        // Same rejection as a wrong password: revealing "this account is locked" would let
        // anyone confirm an email exists just by failing its login a few times.
        let is_locked = user
            .locked_until
            .is_some_and(|until| until > chrono::Utc::now());
        if is_locked {
            outcome(false);
            return Err(AuthError::InvalidCredentials.into());
        }

        let password_ok =
            password::verify_blocking(dto.password, user.password_hash.clone()).await?;
        if !password_ok {
            outcome(false);
            self.users
                .record_failed_login(
                    user.id,
                    self.account.max_failed_login_attempts,
                    Duration::minutes(self.account.account_lockout_minutes),
                )
                .await?;
            return Err(AuthError::InvalidCredentials.into());
        }
        // The password is proven correct, so any earlier wrong attempts were not an attacker's;
        // reset the counter even if a check below still rejects this particular sign-in.
        self.users.reset_failed_logins(user.id).await?;

        if !user.is_active {
            outcome(false);
            return Err(AuthError::AccountDisabled.into());
        }
        if self.account.require_verified_email && !user.is_email_verified() {
            outcome(false);
            return Err(AuthError::EmailNotVerified.into());
        }

        if user.totp_enabled {
            let pending_token = self
                .tokens
                .issue(
                    user.id,
                    TokenPurpose::TwoFactorPending,
                    TWO_FACTOR_PENDING_TTL,
                    None,
                )
                .await?;
            outcome(true);
            return Ok(LoginResponse::two_factor_required(pending_token));
        }

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        outcome(true);
        Ok(LoginResponse::Authenticated(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        }))
    }

    pub async fn me(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        self.users.get(actor, actor.id).await
    }
}
