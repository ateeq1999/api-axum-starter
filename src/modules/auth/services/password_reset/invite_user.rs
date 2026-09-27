//! An administrator invites someone, who then sets their own password.

use super::PasswordResetService;
use crate::{
    common::{error::AppResult, security::Role},
    modules::{
        auth::{dto::InviteUserDto, entity::TokenPurpose, helpers::one_time_token},
        users::dto::UserResponse,
    },
};

impl PasswordResetService {
    /// Admin invitation: creates the account with an unguessable password and emails a link to
    /// choose a real one.
    pub async fn invite(&self, dto: InviteUserDto) -> AppResult<UserResponse> {
        let unusable_password = one_time_token::generate().raw;
        let user = self
            .users
            .create_with_password(
                &dto.email,
                unusable_password,
                dto.display_name.as_deref(),
                dto.role.unwrap_or(Role::User),
                false,
            )
            .await?;

        let raw = self
            .tokens
            .issue(
                user.id,
                TokenPurpose::PasswordReset,
                self.invitation_ttl,
                None,
            )
            .await?;
        self.mail
            .send_invitation(&user.email, &raw, self.invitation_ttl);
        Ok(user.into())
    }
}
