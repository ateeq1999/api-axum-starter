use serde_json::json;
use uuid::Uuid;

use super::{
    dto::{CreateUserDto, ListUsersQuery, UpdateProfileDto, UpdateUserDto, UserResponse},
    entity::User,
    error::UsersError,
    policy,
    repository::{NewUser, UserFilter, UserPatch, UsersRepository},
};
use crate::{
    common::{
        dto::PaginatedResponse,
        error::AppResult,
        security::{
            AuthUser, Role, SecurityError, SessionVerifier, api_key::BoxFuture, password,
            password_policy,
        },
    },
    modules::audit_log::{self, AuditLogService},
};

/// Emails are compared case-insensitively; store them trimmed and lowercased.
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

#[derive(Clone)]
pub struct UsersService {
    repo: UsersRepository,
    /// Whether to also check candidate passwords against the Have I Been Pwned breach database
    /// (see `common::security::password_policy`). Off by default: an external service outage
    /// must never be able to block registration or password changes.
    check_password_breaches: bool,
    /// Reused across breach-check calls rather than built per request (a `reqwest::Client` owns
    /// a connection pool that is meant to be shared).
    breach_check_http_client: reqwest::Client,
    audit_log: AuditLogService,
}

impl UsersService {
    pub fn new(
        repo: UsersRepository,
        check_password_breaches: bool,
        audit_log: AuditLogService,
    ) -> Self {
        Self {
            repo,
            check_password_breaches,
            breach_check_http_client: reqwest::Client::new(),
            audit_log,
        }
    }

    // ---- use-cases exposed over HTTP (users controller) -------------------------------

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

    /// Returns the deleted user's avatar file name, if any, so the caller can remove it —
    /// `UsersService` deliberately knows nothing about avatar storage.
    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<Option<String>> {
        policy::check_delete(actor, id)?;

        let target = self.require(id).await?;
        let released_email = format!("deleted+{id}@deleted.invalid");
        let guard_last_admin = target.role.is_admin() && target.is_active;
        if !self
            .repo
            .soft_delete(id, &released_email, guard_last_admin)
            .await?
        {
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
        self.reject_password_if_policy_violation(&password, &email, display_name)
            .await?;
        let hash = password::hash_blocking(password).await?;
        self.repo
            .create(NewUser {
                email: &email,
                password_hash: &hash,
                display_name,
                role,
                email_verified,
                password_set: true,
            })
            .await
    }

    /// Creates an account that signs in through an external provider and has no password.
    /// The stored hash is not a valid password hash, so no password can ever match it.
    pub async fn create_passwordless(
        &self,
        email: &str,
        display_name: Option<&str>,
        email_verified: bool,
    ) -> AppResult<User> {
        self.repo
            .create(NewUser {
                email: &normalize_email(email),
                password_hash: "!no-password",
                display_name,
                role: Role::User,
                email_verified,
                password_set: false,
            })
            .await
    }

    /// Returns the previous avatar file name (if any) so the caller can delete the file.
    pub async fn set_avatar_key(
        &self,
        id: Uuid,
        avatar_key: Option<&str>,
    ) -> AppResult<Option<String>> {
        Ok(self
            .repo
            .set_avatar_key(id, avatar_key)
            .await?
            .ok_or(UsersError::NotFound)?)
    }

    pub async fn sign_in_method_count(&self, id: Uuid) -> AppResult<i64> {
        self.repo.sign_in_method_count(id).await
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        self.repo.find_by_email(&normalize_email(email)).await
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        self.repo.find_by_id(id).await
    }

    pub async fn set_password(&self, id: Uuid, new_password: String) -> AppResult<()> {
        let account = self.require(id).await?;
        self.reject_password_if_policy_violation(
            &new_password,
            &account.email,
            account.display_name.as_deref(),
        )
        .await?;
        let hash = password::hash_blocking(new_password).await?;
        self.repo.set_password_hash(id, &hash).await
    }

    /// See `UsersRepository::record_failed_login`.
    pub async fn record_failed_login(
        &self,
        id: Uuid,
        max_attempts: i32,
        lockout: chrono::Duration,
    ) -> AppResult<()> {
        self.repo
            .record_failed_login(id, max_attempts, lockout)
            .await
    }

    pub async fn reset_failed_logins(&self, id: Uuid) -> AppResult<()> {
        self.repo.reset_failed_logins(id).await
    }

    pub async fn set_pending_totp_secret(&self, id: Uuid, base32_secret: &str) -> AppResult<()> {
        self.repo.set_pending_totp_secret(id, base32_secret).await
    }

    pub async fn enable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.enable_totp(id).await
    }

    pub async fn disable_totp(&self, id: Uuid) -> AppResult<()> {
        self.repo.disable_totp(id).await
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

    /// Enforces password strength (always) and, if `check_password_breaches` is on, that the
    /// password has not appeared in a known breach. `email`/`display_name` are passed to the
    /// strength check as "known inputs" so a password built from them scores lower than it would
    /// judged in isolation.
    async fn reject_password_if_policy_violation(
        &self,
        candidate_password: &str,
        email: &str,
        display_name: Option<&str>,
    ) -> AppResult<()> {
        let mut known_inputs: Vec<&str> = vec![email];
        if let Some(name) = display_name {
            known_inputs.push(name);
        }
        password_policy::reject_if_too_weak(candidate_password, &known_inputs)
            .map_err(UsersError::PasswordTooWeak)?;

        if self.check_password_breaches
            && password_policy::has_appeared_in_a_known_breach(
                &self.breach_check_http_client,
                candidate_password,
            )
            .await
        {
            return Err(UsersError::PasswordPreviouslyBreached.into());
        }
        Ok(())
    }
}

impl SessionVerifier for UsersService {
    fn verify<'a>(
        &'a self,
        user_id: Uuid,
        token_version: i32,
    ) -> BoxFuture<'a, AppResult<AuthUser>> {
        Box::pin(async move {
            // `find_by_id` already filters out soft-deleted rows.
            let user = self
                .find_by_id(user_id)
                .await?
                .filter(|u| u.is_active)
                .ok_or(SecurityError::InvalidToken)?;
            if user.token_version != token_version {
                return Err(SecurityError::InvalidToken.into());
            }
            Ok(AuthUser::session(user.id, user.role))
        })
    }
}
