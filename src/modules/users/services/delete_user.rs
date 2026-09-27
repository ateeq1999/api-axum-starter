//! An administrator soft-deletes a user.

use serde_json::json;
use uuid::Uuid;

use super::UsersService;
use crate::{
    common::{error::AppResult, security::AuthUser},
    modules::{
        audit_log,
        users::{error::UsersError, policy},
    },
};

impl UsersService {
    /// Returns the deleted user's avatar file name, if any, so the caller can remove it —
    /// `UsersService` deliberately knows nothing about avatar storage.
    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<Option<String>> {
        policy::check_delete(actor, id)?;

        let target = self.require(id).await?;
        let guard_last_admin = target.role.is_admin() && target.is_active;
        if !self.repo.soft_delete(id, guard_last_admin).await? {
            return Err(UsersError::NotFound.into());
        }
        self.audit_log
            .record(
                actor.id,
                audit_log::action::USER_DELETED,
                Some(id),
                json!({ "email": target.email }),
            )
            .await;
        Ok(target.avatar_key)
    }
}
