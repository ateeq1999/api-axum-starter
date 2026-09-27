//! A user changes their own display name.

use super::UsersService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::{
        dto::{UpdateProfileDto, UserResponse},
        error::UsersError,
        repositories::UserPatch,
    },
};

impl UsersService {
    pub async fn update_profile(
        &self,
        actor: &AuthUser,
        dto: UpdateProfileDto,
    ) -> AppResult<UserResponse> {
        let updated = self
            .repo
            .update(
                actor.id,
                UserPatch {
                    display_name: dto.display_name.as_deref(),
                    ..UserPatch::default()
                },
                false,
            )
            .await?
            .ok_or(UsersError::NotFound)?;
        Ok(updated.into())
    }
}
