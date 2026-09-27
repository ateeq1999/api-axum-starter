//! The signed-in account.

use super::SessionService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::users::dto::UserResponse,
};

impl SessionService {
    pub async fn me(&self, actor: &AuthUser) -> AppResult<UserResponse> {
        self.users.get(actor, actor.id).await
    }
}
