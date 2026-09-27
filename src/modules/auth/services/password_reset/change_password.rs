//! Changing the password while signed in.

use super::PasswordResetService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, password},
    },
    modules::{
        auth::{dto::ChangePasswordDto, error::AuthError},
        users::UsersError,
    },
};

impl PasswordResetService {
    pub async fn change(&self, actor: &AuthUser, dto: ChangePasswordDto) -> AppResult<()> {
        let user = self
            .users
            .find_by_id(actor.id)
            .await?
            .ok_or(UsersError::NotFound)?;

        let password_ok =
            password::verify_blocking(dto.current_password, user.password_hash.clone()).await?;
        if !password_ok {
            return Err(AuthError::WrongCurrentPassword.into());
        }

        self.users.set_password(user.id, dto.new_password).await?;
        self.mail.send_password_changed(&user.email);
        Ok(())
    }
}
