//! An administrator creates an account with a password they choose.

use serde_json::json;

use super::UsersService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::{
        audit_log,
        users::dto::{CreateUserDto, UserResponse},
    },
};

impl UsersService {
    pub async fn create(&self, actor: &AuthUser, dto: CreateUserDto) -> AppResult<UserResponse> {
        let assigned_role = dto.role.unwrap_or_default();
        let user = self
            .create_with_password(
                &dto.email,
                dto.password,
                dto.display_name.as_deref(),
                assigned_role,
                false,
            )
            .await?;
        self.audit_log
            .record(
                actor.id,
                audit_log::action::USER_CREATED_BY_ADMIN,
                Some(user.id),
                json!({ "email": user.email, "role": assigned_role }),
            )
            .await;
        Ok(user.into())
    }
}
