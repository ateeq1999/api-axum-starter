//! One user, if the caller may see them.

use uuid::Uuid;

use super::UsersService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::{dto::UserResponse, error::UsersError, policy},
};

impl UsersService {
    pub async fn get(&self, actor: &AuthUser, id: Uuid) -> AppResult<UserResponse> {
        if !policy::can_view(actor, id) {
            return Err(UsersError::Forbidden.into());
        }
        Ok(self.require(id).await?.into())
    }
}
