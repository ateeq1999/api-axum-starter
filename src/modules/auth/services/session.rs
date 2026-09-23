use super::email_verification::EmailVerificationService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, JwtSettings, Role, jwt, password},
    },
    modules::{
        auth::{
            dto::{CredentialsDto, TokenResponse},
            error::AuthError,
        },
        users::{UsersService, dto::UserResponse},
    },
};

#[derive(Clone)]
pub struct SessionService {
    users: UsersService,
    email_verification: EmailVerificationService,
    jwt: JwtSettings,
    require_verified_email: bool,
}

impl SessionService {
    pub fn new(
        users: UsersService,
        email_verification: EmailVerificationService,
        jwt: JwtSettings,
        require_verified_email: bool,
    ) -> Self {
        Self {
            users,
            email_verification,
            jwt,
            require_verified_email,
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

    pub async fn login(&self, dto: CredentialsDto) -> AppResult<TokenResponse> {
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

        let password_ok =
            password::verify_blocking(dto.password, user.password_hash.clone()).await?;
        if !password_ok {
            outcome(false);
            return Err(AuthError::InvalidCredentials.into());
        }
        if !user.is_active {
            outcome(false);
            return Err(AuthError::AccountDisabled.into());
        }
        if self.require_verified_email && !user.is_email_verified() {
            outcome(false);
            return Err(AuthError::EmailNotVerified.into());
        }

        let access_token = jwt::issue(
            user.id,
            user.role,
            user.token_version,
            &self.jwt.secret,
            self.jwt.ttl_secs,
        )?;
        outcome(true);
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: self.jwt.ttl_secs,
        })
    }

    pub async fn me(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        self.users.get(actor, actor.id).await
    }
}
