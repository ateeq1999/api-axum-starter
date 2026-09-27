//! The users business rules, one file per use case. Every file adds methods to the same
//! `UsersService`, so callers (controllers, other features) see one type.

use uuid::Uuid;

use super::{entity::User, error::UsersError, repositories::UsersRepository};
use crate::{
    common::{error::AppResult, security::password_policy},
    modules::audit_log::AuditLogService,
};

mod accounts;
mod avatar;
mod bootstrap_admin;
mod create_user;
mod delete_user;
mod email_addresses;
mod get_user;
mod list_users;
mod login_attempts;
mod session_verifier;
mod sign_in_methods;
mod two_factor;
mod update_profile;
mod update_user;

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

    // ---- helpers shared by the use cases ---------------------------------------------

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
