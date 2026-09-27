//! An administrator changes a user's name, role or active status.

use serde_json::json;
use uuid::Uuid;

use super::UsersService;
use crate::{
    common::{
        error::AppResult,
        security::{AuthUser, Role},
    },
    modules::{
        audit_log,
        users::{
            dto::{UpdateUserDto, UserResponse},
            error::UsersError,
            policy,
            repositories::UserPatch,
        },
    },
};

impl UsersService {
    pub async fn update(
        &self,
        actor: &AuthUser,
        id: Uuid,
        dto: UpdateUserDto,
    ) -> AppResult<UserResponse> {
        policy::check_admin_update(actor, id, &dto)?;
        let before = self.require(id).await?;

        // The authoritative "would this leave zero active admins?" check happens atomically
        // inside `repo.update`, in the same transaction as the write; this only decides whether
        // that check is worth paying for (skipped for e.g. a display-name-only change).
        let guard_last_admin = dto.role == Some(Role::User) || dto.is_active == Some(false);

        let updated = self
            .repo
            .update(
                id,
                UserPatch {
                    display_name: dto.display_name.as_deref(),
                    role: dto.role,
                    is_active: dto.is_active,
                },
                guard_last_admin,
            )
            .await?
            .ok_or(UsersError::NotFound)?;

        if let Some(new_role) = dto.role
            && new_role != before.role
        {
            self.audit_log
                .record(
                    actor.id,
                    audit_log::action::USER_ROLE_CHANGED,
                    Some(id),
                    json!({ "from": before.role, "to": new_role }),
                )
                .await;
        }
        if let Some(now_active) = dto.is_active
            && now_active != before.is_active
        {
            self.audit_log
                .record(
                    actor.id,
                    audit_log::action::USER_ACTIVE_STATUS_CHANGED,
                    Some(id),
                    json!({ "from": before.is_active, "to": now_active }),
                )
                .await;
        }
        Ok(updated.into())
    }
}
