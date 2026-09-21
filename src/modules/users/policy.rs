//! Who may do what to whom. Pure functions so the rules are unit-testable without HTTP.

use uuid::Uuid;

use super::{dto::UpdateUserDto, error::UsersError};
use crate::common::security::{AuthUser, Role};

pub fn can_view(actor: &AuthUser, target: Uuid) -> bool {
    actor.role.is_admin() || actor.id == target
}

/// An administrator may not demote or deactivate their own account (avoids self lock-out).
pub fn check_admin_update(
    actor: &AuthUser,
    target: Uuid,
    patch: &UpdateUserDto,
) -> Result<(), UsersError> {
    let demotes = patch.role == Some(Role::User);
    let deactivates = patch.is_active == Some(false);
    if actor.id == target && (demotes || deactivates) {
        return Err(UsersError::CannotLockOutSelf);
    }
    Ok(())
}

pub fn check_delete(actor: &AuthUser, target: Uuid) -> Result<(), UsersError> {
    if actor.id == target {
        return Err(UsersError::CannotDeleteSelf);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(role: Role) -> AuthUser {
        AuthUser {
            id: Uuid::new_v4(),
            role,
        }
    }

    fn patch(role: Option<Role>, is_active: Option<bool>) -> UpdateUserDto {
        UpdateUserDto {
            display_name: None,
            role,
            is_active,
        }
    }

    #[test]
    fn users_view_only_themselves_admins_view_anyone() {
        let user = actor(Role::User);
        assert!(can_view(&user, user.id));
        assert!(!can_view(&user, Uuid::new_v4()));
        assert!(can_view(&actor(Role::Admin), Uuid::new_v4()));
    }

    #[test]
    fn admin_cannot_lock_themselves_out() {
        let admin = actor(Role::Admin);
        assert!(check_admin_update(&admin, admin.id, &patch(Some(Role::User), None)).is_err());
        assert!(check_admin_update(&admin, admin.id, &patch(None, Some(false))).is_err());
        assert!(
            check_admin_update(&admin, admin.id, &patch(Some(Role::Admin), Some(true))).is_ok()
        );
        assert!(check_admin_update(&admin, Uuid::new_v4(), &patch(Some(Role::User), None)).is_ok());
    }

    #[test]
    fn cannot_delete_self() {
        let admin = actor(Role::Admin);
        assert!(check_delete(&admin, admin.id).is_err());
        assert!(check_delete(&admin, Uuid::new_v4()).is_ok());
    }
}
