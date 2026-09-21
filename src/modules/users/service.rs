use uuid::Uuid;

use super::{
    dto::{CreateUserDto, ListUsersQuery, UpdateProfileDto, UpdateUserDto, UserResponse},
    entity::User,
    error::UsersError,
    policy,
    repository::{NewUser, UserFilter, UserPatch, UsersRepository},
};
use crate::common::{
    dto::PaginatedResponse,
    error::AppResult,
    security::{AuthUser, Role, password},
};

/// Emails are compared case-insensitively; store them trimmed and lowercased.
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

#[derive(Clone)]
pub struct UsersService {
    repo: UsersRepository,
}

impl UsersService {
    pub fn new(repo: UsersRepository) -> Self {
        Self { repo }
    }

    // ---- use-cases exposed over HTTP (users controller) -------------------------------

    pub async fn create(&self, dto: CreateUserDto) -> AppResult<UserResponse> {
        let user = self
            .create_with_password(
                &dto.email,
                dto.password,
                dto.display_name.as_deref(),
                dto.role.unwrap_or_default(),
                false,
            )
            .await?;
        Ok(user.into())
    }

    pub async fn list(&self, query: &ListUsersQuery) -> AppResult<PaginatedResponse<UserResponse>> {
        let pagination = query.pagination();
        let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
        let (users, total) = self
            .repo
            .list(&UserFilter {
                search,
                sort: query.sort.unwrap_or_default(),
                order: query.order.unwrap_or_default(),
                limit: pagination.limit(),
                offset: pagination.offset(),
            })
            .await?;
        Ok(PaginatedResponse::new(
            users.into_iter().map(UserResponse::from).collect(),
            pagination,
            total,
        ))
    }

    pub async fn get(&self, actor: &AuthUser, id: Uuid) -> AppResult<UserResponse> {
        if !policy::can_view(actor, id) {
            return Err(UsersError::Forbidden.into());
        }
        Ok(self.require(id).await?.into())
    }

    pub async fn update(
        &self,
        actor: &AuthUser,
        id: Uuid,
        dto: UpdateUserDto,
    ) -> AppResult<UserResponse> {
        policy::check_admin_update(actor, id, &dto)?;

        let target = self.require(id).await?;
        let loses_admin = target.role.is_admin()
            && target.is_active
            && (dto.role == Some(Role::User) || dto.is_active == Some(false));
        if loses_admin {
            self.ensure_another_admin().await?;
        }

        let updated = self
            .repo
            .update(
                id,
                UserPatch {
                    display_name: dto.display_name.as_deref(),
                    role: dto.role,
                    is_active: dto.is_active,
                },
            )
            .await?
            .ok_or(UsersError::NotFound)?;
        Ok(updated.into())
    }

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
            )
            .await?
            .ok_or(UsersError::NotFound)?;
        Ok(updated.into())
    }

    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<()> {
        policy::check_delete(actor, id)?;

        let target = self.require(id).await?;
        if target.role.is_admin() && target.is_active {
            self.ensure_another_admin().await?;
        }

        let released_email = format!("deleted+{id}@deleted.invalid");
        if !self.repo.soft_delete(id, &released_email).await? {
            return Err(UsersError::NotFound.into());
        }
        Ok(())
    }

    // ---- building blocks used by other modules (auth) ---------------------------------

    pub async fn create_with_password(
        &self,
        email: &str,
        password: String,
        display_name: Option<&str>,
        role: Role,
        email_verified: bool,
    ) -> AppResult<User> {
        let email = normalize_email(email);
        let hash = password::hash_blocking(password).await?;
        self.repo
            .create(NewUser {
                email: &email,
                password_hash: &hash,
                display_name,
                role,
                email_verified,
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
        let hash = password::hash_blocking(new_password).await?;
        self.repo.set_password_hash(id, &hash).await
    }

    pub async fn mark_email_verified(&self, id: Uuid) -> AppResult<()> {
        self.repo.mark_email_verified(id).await
    }

    pub async fn change_email(&self, id: Uuid, new_email: &str) -> AppResult<()> {
        self.repo
            .change_email(id, &normalize_email(new_email))
            .await
    }

    /// Creates the first administrator when none exists. Safe to call on every startup.
    pub async fn ensure_bootstrap_admin(&self, email: &str, password: String) -> AppResult<()> {
        if self.repo.count_active_admins().await? > 0 {
            return Ok(());
        }
        match self
            .create_with_password(email, password, None, Role::Admin, true)
            .await
        {
            Ok(admin) => {
                tracing::info!(email = %admin.email, "bootstrap administrator created");
                Ok(())
            }
            Err(error) => {
                tracing::warn!(?error, "could not create bootstrap administrator");
                Ok(())
            }
        }
    }

    // ---- helpers ----------------------------------------------------------------------

    async fn require(&self, id: Uuid) -> AppResult<User> {
        Ok(self
            .repo
            .find_by_id(id)
            .await?
            .ok_or(UsersError::NotFound)?)
    }

    async fn ensure_another_admin(&self) -> AppResult<()> {
        if self.repo.count_active_admins().await? <= 1 {
            return Err(UsersError::CannotRemoveLastAdmin.into());
        }
        Ok(())
    }
}
