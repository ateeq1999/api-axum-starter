//! Account operations other features build on: sign-up, lookups, password changes.

use uuid::Uuid;

use super::{UsersService, normalize_email};
use crate::{
    common::{
        error::AppResult,
        security::{Role, password},
    },
    modules::users::{entity::User, repositories::NewUser},
};

impl UsersService {
    pub async fn create_with_password(
        &self,
        email: &str,
        password: String,
        display_name: Option<&str>,
        role: Role,
        email_verified: bool,
    ) -> AppResult<User> {
        let email = normalize_email(email);
        self.reject_password_if_policy_violation(&password, &email, display_name)
            .await?;
        let hash = password::hash_blocking(password).await?;
        self.repo
            .create(NewUser {
                email: &email,
                password_hash: &hash,
                display_name,
                role,
                email_verified,
                password_set: true,
            })
            .await
    }

    /// Creates an account that signs in through an external provider and has no password.
    /// The stored hash is not a valid password hash, so no password can ever match it.
    pub async fn create_passwordless(
        &self,
        email: &str,
        display_name: Option<&str>,
        email_verified: bool,
    ) -> AppResult<User> {
        self.repo
            .create(NewUser {
                email: &normalize_email(email),
                password_hash: "!no-password",
                display_name,
                role: Role::User,
                email_verified,
                password_set: false,
            })
            .await
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        self.repo.find_by_email(&normalize_email(email)).await
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        self.repo.find_by_id(id).await
    }

    pub async fn set_password(&self, id: Uuid, new_password: String) -> AppResult<()> {
        let account = self.require(id).await?;
        self.reject_password_if_policy_violation(
            &new_password,
            &account.email,
            account.display_name.as_deref(),
        )
        .await?;
        let hash = password::hash_blocking(new_password).await?;
        self.repo.set_password_hash(id, &hash).await
    }
}
