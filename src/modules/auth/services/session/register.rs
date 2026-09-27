//! Creating an account with a password, and sending the verification email.

use super::SessionService;
use crate::{
    common::{error::AppResult, security::Role},
    modules::{auth::dto::CredentialsDto, users::dto::UserResponse},
};

impl SessionService {
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
}
